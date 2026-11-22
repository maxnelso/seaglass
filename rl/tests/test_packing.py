"""Packed trajectories: CSR slot features, bit-packed masks, and the trainers that consume them."""

from __future__ import annotations

import random

import numpy as np
import pytest
import torch

from seaglass_rl import packing
from seaglass_rl.actions import ACTION_SPACE_SIZE
from seaglass_rl.features import NUM_SLOTS, SLOT_NUM_DIM
from seaglass_rl.model import BattlegroundsMLP
from seaglass_rl.play import GameSpec, LobbyConfig, PlayOptions, play_games
from seaglass_rl.ppo import build_batch, ppo_update
from seaglass_rl import pretrain

CONFIG = LobbyConfig(players=4, max_turns=4)
SPECS = [GameSpec(seed=s, controllers=["learner", "9kgamerbot"] * 2) for s in (1, 2)]


def make_model() -> BattlegroundsMLP:
    torch.manual_seed(0)  # the weights are initialised from torch's global RNG
    return BattlegroundsMLP(hidden_dim=32, card_embed_dim=4, value_places=4, kind_prior=True, seed=0)


def play(pack: bool, model=None, record=("learner",)):
    model = model or make_model()
    torch.manual_seed(0)
    np.random.seed(0)
    random.seed(0)
    return [t for r in play_games(CONFIG, SPECS, {"learner": model}, PlayOptions(pack=pack), record=record) for t in r.trajectories]


@pytest.fixture(scope="module")
def pair():
    # The first game played in a process differs from later ones with the same seeds (lazy state
    # somewhere in the engine or bots), so warm up before comparing.
    play(False)
    return play(False), play(True)


def test_roundtrip_on_real_trajectories(pair) -> None:
    dense, packed = pair
    assert len(dense) == len(packed) > 0
    for d, p in zip(dense, packed):
        assert bool(p["packed"]) and "slot_num_feats" not in p and "action_mask" not in p
        rows = np.arange(len(d["actions"]))
        got = packing.unpack_rows(p["num_ptr"], p["num_idx"], p["num_val"], rows)
        assert got.dtype == np.float32 and got.shape == (len(rows), NUM_SLOTS, SLOT_NUM_DIM)
        assert np.array_equal(got, d["slot_num_feats"].astype(np.float32))
        assert np.array_equal(packing.unpack_mask(p["mask_bits"], rows), d["action_mask"])
        assert np.array_equal(p["slot_card_ids"].astype(np.int32), d["slot_card_ids"])
        assert p["slot_card_ids"].dtype == np.uint16 and p["num_idx"].dtype == np.uint16
        for k in ("player_vec", "actions", "log_probs", "values", "rewards", "turn_end", "place"):
            assert np.array_equal(p[k], d[k])
        assert sum(a.nbytes for a in p.values()) < sum(a.nbytes for a in d.values()) / 3


def test_unpack_subset_and_repeats(pair) -> None:
    dense, packed = pair
    d, p = dense[0], packed[0]
    rng = np.random.default_rng(0)
    rows = rng.integers(0, len(d["actions"]), size=50)  # unordered, with repeats
    got = packing.unpack_rows(p["num_ptr"], p["num_idx"], p["num_val"], rows)
    assert np.array_equal(got, d["slot_num_feats"][rows].astype(np.float32))
    assert np.array_equal(packing.unpack_mask(p["mask_bits"], rows), d["action_mask"][rows])
    empty = packing.unpack_rows(p["num_ptr"], p["num_idx"], p["num_val"], np.array([], dtype=np.int64))
    assert empty.shape == (0, NUM_SLOTS, SLOT_NUM_DIM)


def test_all_zero_rows_and_concat() -> None:
    rng = np.random.default_rng(1)
    parts, dense = [], []
    for T in (3, 1, 5):
        x = np.zeros((T, NUM_SLOTS, SLOT_NUM_DIM), dtype=np.float16)
        x[rng.random(x.shape) < 0.02] = 1.5
        x[0] = 0  # an empty row
        dense.append(x)
        parts.append(packing.pack_rows(x))
    ptr, idx, val = packing.concat_packed(parts)
    assert ptr[0] == 0 and len(ptr) == 10 and ptr[-1] == len(idx) == len(val)
    got = packing.unpack_rows(ptr, idx, val, np.arange(9))
    assert np.array_equal(got, np.concatenate(dense).astype(np.float32))


def test_mask_roundtrip() -> None:
    m = np.random.default_rng(2).random((7, ACTION_SPACE_SIZE)) < 0.1
    assert np.array_equal(packing.unpack_mask(packing.pack_mask(m), np.array([6, 0, 0, 3])), m[[6, 0, 0, 3]])


def test_mixing_raises(pair) -> None:
    dense, packed = pair
    with pytest.raises(ValueError, match="packed and dense"):
        build_batch([dense[0], packed[0]], 0.99, 0.95)


def test_build_batch_and_update_match(pair) -> None:
    dense, packed = pair
    bd, bp = build_batch(dense, 0.99, 0.95), build_batch(packed, 0.99, 0.95)
    for k in ("advantages", "returns", "places", "actions", "old_log_probs", "old_values", "player_vec", "slot_card_ids"):
        assert torch.equal(bd[k], bp[k]), k
    n = len(bd["actions"])
    rows = np.random.default_rng(3).permutation(n)[:64]
    assert torch.equal(
        bd["slot_num_feats"][rows].float(),
        torch.from_numpy(packing.unpack_rows(bp["num_ptr"], bp["num_idx"], bp["num_val"], rows)),
    )
    stats = []
    for b in (bd, bp):
        model = make_model()
        opt = torch.optim.Adam(model.parameters(), lr=1e-3)
        torch.manual_seed(5)
        stats.append(ppo_update(model, opt, b, epochs=1, minibatch_size=32, target_kl=None))
    for k in stats[0]:
        assert stats[0][k] == pytest.approx(stats[1][k], abs=1e-6), k


def test_pretrain_on_packed_dataset() -> None:
    config = LobbyConfig(players=4, max_turns=3)
    opts = PlayOptions()
    packed = pretrain.collect(config, 4, 0, opts, seed=7, chunk=2)
    dense = pretrain.collect(config, 4, 0, opts, seed=7, chunk=2, pack=False)
    assert "num_ptr" in packed and "slot_num_feats" not in packed and "slot_num_feats" in dense
    n = len(dense["actions"])
    assert len(packed["num_ptr"]) == n + 1 and np.array_equal(packed["game"], dense["game"])
    rows = np.arange(n)
    assert np.array_equal(
        packing.unpack_rows(packed["num_ptr"], packed["num_idx"], packed["num_val"], rows),
        dense["slot_num_feats"].astype(np.float32),
    )
    stats = []
    for data in (packed, dense):
        model = make_model()
        torch.manual_seed(0)
        stats.append(pretrain.train(model, data, epochs=1, lr=1e-3, minibatch=64, value_epochs=0))
    assert stats[0]["accuracy"] == pytest.approx(stats[1]["accuracy"], abs=1e-6)
