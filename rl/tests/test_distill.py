"""Distilling the search into the network (`distill.py`)."""

from __future__ import annotations

import json

import numpy as np
import pytest
import torch

from seaglass_rl import checkpoint, distill
from seaglass_rl.actions import ACTION_SPACE_SIZE
from seaglass_rl.search import SearchSpec
from seaglass_rl.transformer import EntityTransformer

SMALL = dict(d_model=32, layers=2, heads=2, ff=64, head_dim=32, card_embed_dim=8)
SPEC = SearchSpec(top_m=2, sims=2)


def _model(places: int = 8, seed: int = 0) -> EntityTransformer:
    torch.manual_seed(seed)
    m = EntityTransformer(seed=seed, kind_prior=True, value_places=places, **SMALL)
    m.eval()
    return m


@pytest.fixture(scope="module")
def generated(tmp_path_factory):
    root = tmp_path_factory.mktemp("distill")
    ckpt = root / "init.pt"
    checkpoint.save(ckpt, _model(), args={"max_steps_per_turn": 30})
    meta = distill.generate(str(ckpt), root / "data", lobbies=2, search_seats=2, workers=0, seed=123, chunk=1, spec=SPEC)
    return ckpt, root / "data", meta


def test_generate_writes_shards_and_meta(generated) -> None:
    _, data_dir, meta = generated
    assert len(list(data_dir.glob("shard_*.npz"))) == 2
    on_disk = json.loads((data_dir / "meta.json").read_text())
    assert on_disk["lobbies"] == 2 and on_disk["search"]["gumbel_noise"] is True and on_disk["engine"] == meta["engine"]
    data = distill.load_data(data_dir)
    n = len(data["actions"])
    assert n == meta["decisions"] > 0
    assert data["search_policy"].shape == (n, ACTION_SPACE_SIZE) and data["search_policy"].dtype == np.float16
    for k in ("searched", "places", "game", "player_vec", "slot_card_ids", "mask_bits"):
        assert len(data[k]) == n
    assert len(data["num_ptr"]) == n + 1


def test_targets_places_and_games(generated) -> None:
    _, data_dir, _ = generated
    data = distill.load_data(data_dir)
    mask = np.unpackbits(data["mask_bits"], axis=1, count=ACTION_SPACE_SIZE).astype(bool)
    sp = data["search_policy"].astype(np.float32)
    s = data["searched"]
    assert s.any()
    np.testing.assert_allclose(sp[s].sum(1), 1.0, atol=5e-3)
    assert not (sp[s][~mask[s]] > 0).any()
    assert not sp[~s].any()
    # 2 lobbies, 2 searchers each: each game's rows form 2 seats, and places are in 0..7.
    assert sorted(np.unique(data["game"])) == [0, 1]
    assert data["places"].min() >= 0 and data["places"].max() <= 7
    for g in (0, 1):
        assert len(np.unique(data["places"][data["game"] == g])) <= 2


def test_places_match_the_games_placements(tmp_path) -> None:
    from seaglass_rl.play import GameSpec, LobbyConfig, Player

    model = _model()
    ckpt = tmp_path / "m.pt"
    checkpoint.save(ckpt, model)
    distill.generate(str(ckpt), tmp_path / "d", lobbies=1, search_seats=2, seed=55, chunk=1, spec=SPEC)
    data = distill.load_data(tmp_path / "d")
    # Replay the same lobby (generate shuffles seats with random.Random(seed)).
    import dataclasses
    import random

    seats = ["searcher"] * 2 + ["rival"] * 6
    random.Random(55).shuffle(seats)
    from seaglass_rl.ladder import checkpoint_options

    opts = dataclasses.replace(checkpoint_options(str(ckpt)), search=dataclasses.replace(SPEC, gumbel_noise=True), search_controllers=("searcher",), pack=True)
    (res,) = Player(0).play(LobbyConfig(), [GameSpec(seed=55, controllers=seats)], {"searcher": str(ckpt), "rival": str(ckpt)}, opts, record=("searcher",), seed=55)
    expected = sorted(int(p) - 1 for u, p in enumerate(res.placements) if seats[u] == "searcher")
    assert sorted(set(data["places"].tolist())) == sorted(set(expected))


def test_train_saves_a_loadable_checkpoint(generated, tmp_path) -> None:
    ckpt, data_dir, _ = generated
    out = tmp_path / "student.pt"
    hist = distill.run_train(str(ckpt), str(data_dir), str(out), epochs=1, lr=1e-4, minibatch=64, holdout=0.5, device="cpu")
    assert [h["epoch"] for h in hist] == [0, 1]
    model, c = checkpoint.load(out)
    assert c["args"] == {"max_steps_per_turn": 30}
    d = c["extra"]["distill"]
    assert d["epochs"] == 1 and "kl_to_init" in d["metrics"] and "agree" in d["metrics"]


def _batch(model, n: int = 16, seed: int = 0):
    rng = np.random.default_rng(seed)
    from seaglass_rl.features import NUM_SLOTS, PLAYER_DIM, SLOT_NUM_DIM, VOCAB_SIZE

    mask = torch.zeros(n, ACTION_SPACE_SIZE, dtype=torch.bool)
    legal = rng.choice(ACTION_SPACE_SIZE, size=(n, 5))
    for i in range(n):
        mask[i, legal[i]] = True
    target = torch.zeros(n, ACTION_SPACE_SIZE)
    chosen = torch.from_numpy(legal[:, 0])
    target[torch.arange(n), chosen] = 1.0
    return {
        "player_vec": torch.randn(n, PLAYER_DIM),
        "card_ids": torch.randint(0, VOCAB_SIZE, (n, NUM_SLOTS)),
        "num_feats": torch.randn(n, NUM_SLOTS, SLOT_NUM_DIM) * 0.1,
        "mask": mask,
        "target": target,
        "searched": torch.ones(n, dtype=torch.bool),
        "places": torch.randint(0, 8, (n,)),
    }, chosen


def _steps(model, b, policy_coef: float, anchor_coef: float, steps: int = 20, lr: float = 1e-3):
    import copy

    init = copy.deepcopy(model).eval().requires_grad_(False)
    opt = torch.optim.Adam(model.parameters(), lr=lr)
    model.train()
    for _ in range(steps):
        with torch.no_grad():
            il, _ = distill._forward(init, b)
        t = distill.loss_terms(model, il, b)
        loss = policy_coef * t["policy"] + anchor_coef * t["kl"]
        opt.zero_grad()
        loss.backward()
        opt.step()
    model.eval()
    with torch.no_grad():
        il, _ = distill._forward(init, b)
        return distill.loss_terms(model, il, b)


def test_policy_loss_raises_the_target_action() -> None:
    model = _model()
    b, chosen = _batch(model)
    with torch.no_grad():
        before = torch.softmax(distill._forward(model, b)[0], -1)[torch.arange(16), chosen].mean()
    _steps(model, b, 1.0, 0.0, steps=40)
    with torch.no_grad():
        after = torch.softmax(distill._forward(model, b)[0], -1)[torch.arange(16), chosen].mean()
    assert after > before + 0.2


def test_anchor_alone_keeps_the_kl_near_zero() -> None:
    model = _model()
    b, _ = _batch(model)
    t = _steps(model, b, 0.0, 1.0, steps=10)
    assert float(t["kl"]) < 1e-4
    # With the policy pulling and no anchor, it moves; the anchor holds it back.
    free = _steps(_model(), b, 1.0, 0.0, steps=10)
    held = _steps(_model(), b, 1.0, 5.0, steps=10)
    assert float(free["kl"]) > float(held["kl"]) > 0


def test_scalar_critic_is_refused(generated, tmp_path) -> None:
    _, data_dir, _ = generated
    ckpt = tmp_path / "scalar.pt"
    checkpoint.save(ckpt, _model(places=0))
    with pytest.raises(ValueError, match="scalar critic"):
        distill.run_train(str(ckpt), str(data_dir), str(tmp_path / "o.pt"), epochs=1)


def test_a_bigger_student_trains_from_the_teacher(generated, tmp_path) -> None:
    ckpt, data_dir, _ = generated
    out = tmp_path / "student.pt"
    # Two data directories (the same one twice) are concatenated with distinct games.
    both = f"{data_dir},{data_dir}"
    one = distill.load_data(str(data_dir))
    two = distill.load_data(both)
    assert len(two["actions"]) == 2 * len(one["actions"])
    assert len(np.unique(two["game"])) == 2 * len(np.unique(one["game"]))
    hist = distill.run_train(
        str(ckpt), both, str(out), epochs=1, lr=1e-3, minibatch=64, holdout=0.5, device="cpu",
        student="48,2,2,96", anchor_coef=1.0,
    )
    model, saved = checkpoint.load(out)
    assert model.config["d_model"] == 48 and model.config["layers"] == 2
    assert model.config["value_places"] == 8
    assert saved["extra"]["distill"]["student"] == "48,2,2,96"
    assert np.isfinite(hist[-1]["policy_ce"]) and hist[-1]["kl_to_init"] > 0
