"""The auxiliary head: probe targets by decision, the head, the loss, the flags."""

from __future__ import annotations

import numpy as np
import pytest
import torch

from seaglass_rl import checkpoint
from seaglass_rl.model import BattlegroundsMLP
from seaglass_rl.play import GameSpec, LobbyConfig, PlayOptions, play_games
from seaglass_rl.ppo import build_batch, ppo_update
from seaglass_rl.probe import ProbeSpec
from seaglass_rl.transformer import EntityTransformer

SMALL_PROBE = ProbeSpec(lobbies=4, per_turn=8, min_boards=4)
SMALL = dict(d_model=32, layers=2, heads=2, ff=64, head_dim=32, card_embed_dim=8)
HORIZONS = (0, 2, 4)


def lobby(**kw):
    config = LobbyConfig(players=4, max_turns=8)
    opts = PlayOptions(aux_probe=SMALL_PROBE, aux_horizons=HORIZONS, turn_stats=True, probe=SMALL_PROBE, **kw)
    spec = GameSpec(seed=3, controllers=["heuristic"] * 4, probe_units=(0, 1, 2, 3))
    return play_games(config, [spec], {}, opts, record=("heuristic",))[0]


def test_targets_follow_the_turns_probe_scores() -> None:
    res = lobby()
    assert res.trajectories
    for p, tr in enumerate(res.trajectories):
        T = len(tr["actions"])
        y, m = tr["aux_target"], tr["aux_mask"]
        assert y.shape == (T, 3) and y.dtype == np.float32 and m.shape == (T, 3) and m.dtype == np.bool_
        assert ((y >= 0) & (y <= 1)).all() and (y[~m] == 0).all()
        # Every decision has a turn: the turn's decisions end at `turn_end`; the hero phase comes first.
        stats = {s.turn: s.probe for s in res.turn_stats[p]}
        probe = {t: v for t, v in stats.items() if v is not None}
        assert probe
        assert (~m[0]).all()  # hero phase (turn 0): no targets
        # y_0 is the turn's own stat for decisions that have one; decisions share targets within a turn.
        seen = 0
        for i in range(T):
            if not m[i, 0]:
                continue
            matches = [t for t, v in probe.items() if np.isclose(v, y[i, 0], atol=1e-6)]
            assert matches
            seen += 1
        assert seen
        # After the seat's last turn there is no target: the last decisions have mask 0 for k >= 1.
        assert not m[-1, 1:].any()
        # Decisions sharing a turn share targets.
        groups = np.cumsum(np.r_[0, tr["turn_end"][:-1].astype(int)])
        for gid in np.unique(groups):
            rows = np.nonzero(groups == gid)[0]
            assert (y[rows] == y[rows[0]]).all() and (m[rows] == m[rows[0]]).all()


def test_y0_equals_the_turn_stat_probe_per_turn() -> None:
    res = lobby()
    for p, tr in enumerate(res.trajectories):
        stats = [s for s in res.turn_stats[p] if s.probe is not None]
        groups = np.cumsum(np.r_[0, tr["turn_end"][:-1].astype(int)])
        have = [g for g in np.unique(groups) if tr["aux_mask"][groups == g, 0].all()]
        # One turn per group of decisions with a y_0, in order, each equal to that turn's stat probe.
        assert len(have) == len(stats) > 0
        for g, s in zip(have, stats):
            assert np.allclose(tr["aux_target"][groups == g, 0], s.probe, atol=1e-6)
        # y_k is the stat probe k turns on, where the seat lived and the library has the turn.
        by_turn = {s.turn: s.probe for s in stats}
        turns = [s.turn for s in stats]
        for g, t in zip(have, turns):
            rows = groups == g
            for j, k in enumerate(HORIZONS):
                want = by_turn.get(t + k)
                assert tr["aux_mask"][rows, j].all() == (want is not None)
                if want is not None:
                    assert np.allclose(tr["aux_target"][rows, j], want, atol=1e-6)


def test_aux_off_adds_no_arrays_and_duos_raise() -> None:
    (res,) = play_games(LobbyConfig(players=4, max_turns=4), [GameSpec(seed=1, controllers=["heuristic"] * 4)], {}, record=("heuristic",))
    assert all("aux_target" not in tr for tr in res.trajectories)
    with pytest.raises(ValueError):
        play_games(
            LobbyConfig(duos=True, players=8, max_turns=4),
            [GameSpec(seed=1, controllers=["heuristic"] * 4)],
            {},
            PlayOptions(aux_probe=SMALL_PROBE),
            record=("heuristic",),
        )


def test_packed_and_dense_carry_identical_aux_arrays() -> None:
    config = LobbyConfig(players=4, max_turns=6)
    spec = GameSpec(seed=5, controllers=["heuristic"] * 4)
    dense = play_games(config, [spec], {}, PlayOptions(aux_probe=SMALL_PROBE), record=("heuristic",))[0]
    packed = play_games(config, [spec], {}, PlayOptions(aux_probe=SMALL_PROBE, pack=True), record=("heuristic",))[0]
    assert len(dense.trajectories) == len(packed.trajectories)
    for d, p in zip(dense.trajectories, packed.trajectories):
        assert "aux_target" in d and "aux_target" in p
        np.testing.assert_array_equal(d["aux_target"], p["aux_target"])
        np.testing.assert_array_equal(d["aux_mask"], p["aux_mask"])


def make(arch: str, aux: int):
    if arch == "mlp":
        return BattlegroundsMLP(hidden_dim=32, card_embed_dim=4, seed=0, value_places=4, aux_heads=aux)
    return EntityTransformer(seed=0, value_places=4, aux_heads=aux, **SMALL)


def args_for(batch: int = 3):
    config = LobbyConfig(players=4, max_turns=4)
    (res,) = play_games(config, [GameSpec(seed=2, controllers=["heuristic"] * 4)], {}, record=("heuristic",))
    tr = res.trajectories[0]
    return (
        torch.from_numpy(tr["player_vec"][:batch]),
        torch.from_numpy(tr["slot_card_ids"][:batch].astype(np.int64)),
        torch.from_numpy(tr["slot_num_feats"][:batch].astype(np.float32)),
        torch.from_numpy(tr["action_mask"][:batch]),
    )


@pytest.mark.parametrize("arch", ["mlp", "transformer"])
def test_no_aux_head_leaves_the_state_dict_unchanged(arch: str) -> None:
    keys0 = set(make(arch, 0).state_dict())
    keys3 = set(make(arch, 3).state_dict())
    assert keys3 - keys0 == {"aux.weight", "aux.bias"} and keys0 < keys3
    assert not any(k.startswith("aux") for k in keys0)
    old = make(arch, 0).state_dict()
    make(arch, 0).load_state_dict(old)  # old checkpoints load


@pytest.mark.parametrize("arch", ["mlp", "transformer"])
def test_forward_shapes_and_checkpoint_round_trip(arch: str, tmp_path) -> None:
    args = args_for()
    m = make(arch, 3).eval()
    with torch.no_grad():
        plain = m(*args)
        assert len(plain) == 2
        assert len(m(*args, return_value_logits=True)) == 3
        for vl in (False, True):
            out = m(*args, return_value_logits=vl, return_aux=True)
            assert len(out) == 4 and out[2].shape == (3, 4) and out[3].shape == (3, 3)
        assert make(arch, 0)(*args, return_aux=True)[3] is None
    path = tmp_path / "m.pt"
    checkpoint.save(path, m)
    m2, _ = checkpoint.load(path)
    assert m2.config["aux_heads"] == 3
    with torch.no_grad():
        assert torch.allclose(m2(*args, return_aux=True)[3], m(*args, return_aux=True)[3])


def aux_batch():
    config = LobbyConfig(players=4, max_turns=6)
    specs = [GameSpec(seed=s, controllers=["heuristic"] * 4) for s in (1, 2)]
    res = play_games(config, specs, {}, PlayOptions(aux_probe=SMALL_PROBE, pack=True), record=("heuristic",))
    return build_batch([t for r in res for t in r.trajectories], 1.0, 0.95)


@pytest.mark.parametrize("arch", ["mlp", "transformer"])
def test_aux_loss_trains_only_the_aux_head(arch: str) -> None:
    batch = aux_batch()
    assert batch["aux_target"].shape[1] == 3 and batch["aux_mask"].any()

    def run(coef: float):
        torch.manual_seed(0)
        m = make(arch, 3)
        before = m.aux.weight.detach().clone()
        opt = torch.optim.SGD(m.parameters(), lr=0.1)
        stats = ppo_update(m, opt, batch, epochs=1, minibatch_size=64, aux_coef=coef, target_kl=None)
        return stats, (m.aux.weight.detach() - before).abs().max().item()

    stats, moved = run(0.1)
    assert np.isfinite(stats["aux_loss"]) and stats["aux_loss"] > 0 and moved > 0
    stats0, moved0 = run(0.0)
    assert stats0["aux_loss"] == 0.0 and moved0 == 0.0


def test_an_all_masked_batch_has_zero_aux_loss() -> None:
    batch = aux_batch()
    batch["aux_mask"] = torch.zeros_like(batch["aux_mask"])
    m = make("mlp", 3)
    stats = ppo_update(m, torch.optim.SGD(m.parameters(), lr=0.1), batch, epochs=1, aux_coef=0.1, target_kl=None)
    assert stats["aux_loss"] == 0.0


def test_train_lobby_smoke_with_aux(tmp_path) -> None:
    import json

    from seaglass_rl import train_lobby

    run = tmp_path / "run"
    base = ["--players", "8", "--games", "2", "--workers", "0", "--threads", "1", "--eval-ladder", "heuristic",
            "--eval-games", "2", "--max-turns", "6", "--device", "cpu", "--minibatch", "256"]
    train_lobby.main(["--run-dir", str(run), "--iterations", "2", "--aux-coef", "0.1", *base])
    rows = [json.loads(line) for line in (run / "log.jsonl").read_text().splitlines()]
    assert len(rows) == 2 and all(r["aux_loss"] > 0 for r in rows)
    model, _ = checkpoint.load(run / "latest.pt")
    assert model.config["aux_heads"] == 3
    # A checkpoint with other heads is refused, not grown.
    with pytest.raises(SystemExit):
        train_lobby.main(["--run-dir", str(tmp_path / "r2"), "--iterations", "1", "--aux-coef", "0.1",
                          "--aux-horizons", "0,1", "--init", str(run / "latest.pt"), *base])
