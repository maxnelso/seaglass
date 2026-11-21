"""Stage 2 defaults: turn-level GAE, the placement critic, final places, the league."""

from __future__ import annotations

import random

import numpy as np
import pytest
import torch

from seaglass_rl import checkpoint
from seaglass_rl.model import BattlegroundsMLP
from seaglass_rl.play import GameSpec, LobbyConfig, PlayOptions, placement_reward, play_games
from seaglass_rl.ppo import build_batch, gae, ppo_update
from seaglass_rl.transformer import EntityTransformer

SMALL = dict(d_model=32, layers=2, heads=2, ff=64, head_dim=32, card_embed_dim=8)


def old_gae(rewards, values, turn_end, done, gamma, lam):
    T = len(rewards)
    adv = np.zeros(T, dtype=np.float32)
    last = 0.0
    for t in reversed(range(T)):
        g = gamma if turn_end[t] else 1.0
        next_value = (0.0 if done else values[t]) if t == T - 1 else values[t + 1]
        delta = rewards[t] + g * next_value - values[t]
        last = delta + g * lam * last
        adv[t] = last
    return adv, adv + values


def test_gae_turn_applies_lambda_only_at_turn_ends() -> None:
    # Two turns of two actions each; the reward comes at the end.
    rewards = np.array([0, 0, 0, 1.0], dtype=np.float32)
    values = np.array([0.1, 0.2, 0.3, 0.4], dtype=np.float32)
    turn_end = np.array([False, True, False, True])
    gamma, lam = 0.9, 0.5
    adv, ret = gae(rewards, values, turn_end, True, gamma, lam)
    d3 = 1.0 - 0.4
    a3 = d3
    a2 = (0.0 + 1.0 * 0.4 - 0.3) + 1.0 * a3  # within a turn: g = 1, l = 1
    d1 = 0.0 + gamma * 0.3 - 0.2
    a1 = d1 + gamma * lam * a2  # at the turn end
    a0 = (0.0 + 1.0 * 0.2 - 0.1) + 1.0 * a1
    assert np.allclose(adv, [a0, a1, a2, a3], atol=1e-6)
    assert np.allclose(ret, adv + values)


def test_gae_per_action_matches_the_old_implementation() -> None:
    rng = np.random.default_rng(0)
    for done in (True, False):
        T = 40
        rewards = rng.normal(size=T).astype(np.float32)
        values = rng.normal(size=T).astype(np.float32)
        turn_end = rng.random(T) < 0.3
        a, r = gae(rewards, values, turn_end, done, 0.97, 0.95, lam_per="action")
        oa, orr = old_gae(rewards, values, turn_end, done, 0.97, 0.95)
        assert np.allclose(a, oa) and np.allclose(r, orr)
    with pytest.raises(ValueError):
        gae(rewards, values, turn_end, True, 1.0, 0.95, lam_per="step")


@pytest.mark.parametrize("k", [0, 8])
def test_mlp_numpy_act_batch_matches_torch_value(k: int) -> None:
    model = BattlegroundsMLP(hidden_dim=32, card_embed_dim=4, value_places=k, kind_prior=True, seed=1)
    config = LobbyConfig(players=4, max_turns=4)
    (res,) = play_games(config, [GameSpec(seed=1, controllers=["learner", "heuristic"] * 2)], {"learner": model})
    tr = res.trajectories[0]
    p, c, n, m = tr["player_vec"][:20], tr["slot_card_ids"][:20], tr["slot_num_feats"][:20].astype(np.float32), tr["action_mask"][:20]
    _, _, vals = model.act_batch(p, c.astype(np.int64), n, m, deterministic=True)
    with torch.no_grad():
        _, value = model(torch.from_numpy(p), torch.from_numpy(c.astype(np.int64)), torch.from_numpy(n), torch.from_numpy(m))
    assert np.allclose(vals, value.numpy(), atol=1e-5)


def test_placement_critic_value_is_the_expectation() -> None:
    model = BattlegroundsMLP(hidden_dim=32, card_embed_dim=4, value_places=8, seed=0)
    assert np.allclose(model.value_atoms.numpy(), [placement_reward(p, 8) for p in range(1, 9)])
    assert BattlegroundsMLP(value_places=1).value_atoms.tolist() == [0.0]
    B = 3
    from seaglass_rl.features import NUM_SLOTS, PLAYER_DIM, SLOT_NUM_DIM

    args = (torch.randn(B, PLAYER_DIM), torch.zeros(B, NUM_SLOTS, dtype=torch.long), torch.randn(B, NUM_SLOTS, SLOT_NUM_DIM))
    logits, value = model(*args)
    logits2, value2, vlogits = model(*args, return_value_logits=True)
    assert vlogits.shape == (B, 8) and torch.equal(value, value2)
    assert torch.allclose(value, torch.softmax(vlogits, -1) @ model.value_atoms)
    assert BattlegroundsMLP(hidden_dim=32, card_embed_dim=4)(*args, return_value_logits=True)[2] is None


def test_scalar_state_dict_keys_are_unchanged() -> None:
    for cls, kw in ((BattlegroundsMLP, {}), (EntityTransformer, SMALL)):
        base = cls(**kw)
        explicit = cls(value_places=0, **kw)
        assert list(base.state_dict()) == list(explicit.state_dict())
        assert all(a.shape == b.shape for a, b in zip(base.state_dict().values(), explicit.state_dict().values()))
    assert BattlegroundsMLP().critic.weight.shape[0] == 1


def test_transformer_with_a_placement_critic_runs() -> None:
    model = EntityTransformer(seed=0, kind_prior=True, value_places=8, **SMALL)
    config = LobbyConfig(players=8, max_turns=3)
    (res,) = play_games(config, [GameSpec(seed=2, controllers=["learner"] + ["heuristic"] * 7)], {"learner": model})
    tr = res.trajectories[0]
    assert len(tr["values"]) > 0 and np.all(np.abs(tr["values"]) <= 1.0 + 1e-6)
    logits, value, vl = model(
        torch.from_numpy(tr["player_vec"][:5]),
        torch.from_numpy(tr["slot_card_ids"][:5].astype(np.int64)),
        torch.from_numpy(tr["slot_num_feats"][:5].astype(np.float32)),
        torch.from_numpy(tr["action_mask"][:5]),
        return_value_logits=True,
    )
    assert vl.shape == (5, 8)


@pytest.mark.parametrize("arch", ["mlp", "transformer"])
def test_placement_checkpoint_round_trip(arch: str, tmp_path) -> None:
    if arch == "mlp":
        model = BattlegroundsMLP(hidden_dim=32, card_embed_dim=4, value_places=8, seed=3)
    else:
        model = EntityTransformer(seed=3, value_places=8, **SMALL)
    path = tmp_path / "m.pt"
    checkpoint.save(path, model)
    loaded, _ = checkpoint.load(path)
    assert loaded.config["value_places"] == 8
    config = LobbyConfig(players=8, max_turns=3)
    (a,) = play_games(config, [GameSpec(seed=5, controllers=["learner"] + ["heuristic"] * 7)], {"learner": model})
    (b,) = play_games(config, [GameSpec(seed=5, controllers=["learner"] + ["heuristic"] * 7)], {"learner": loaded})
    # Sampling differs by rng seed, so compare the values of the same observations.
    obs = a.trajectories[0]
    args = (
        torch.from_numpy(obs["player_vec"][:4]),
        torch.from_numpy(obs["slot_card_ids"][:4].astype(np.int64)),
        torch.from_numpy(obs["slot_num_feats"][:4].astype(np.float32)),
        torch.from_numpy(obs["action_mask"][:4]),
    )
    with torch.no_grad():
        assert torch.allclose(model(*args)[1], loaded(*args)[1])
    assert b.trajectories


def test_play_games_records_final_places() -> None:
    model = BattlegroundsMLP(hidden_dim=32, card_embed_dim=4, kind_prior=True)
    config = LobbyConfig(players=8, max_turns=30)
    ctrl = ["learner", "heuristic"] * 4
    (res,) = play_games(config, [GameSpec(seed=3, controllers=ctrl)], {"learner": model})
    assert len(res.trajectories) == 4
    places = sorted(int(t["place"]) for t in res.trajectories)
    assert all(1 <= p <= 8 for p in places)
    assert places == sorted(res.placements[u] for u in range(8) if ctrl[u] == "learner")


def test_ppo_update_with_a_placement_critic() -> None:
    model = BattlegroundsMLP(hidden_dim=32, card_embed_dim=4, kind_prior=True, value_places=8)
    config = LobbyConfig(players=8, max_turns=6)
    ctrl = ["learner", "heuristic"] * 4
    results = play_games(config, [GameSpec(seed=s, controllers=ctrl) for s in (1, 2)], {"learner": model})
    batch = build_batch([t for r in results for t in r.trajectories], gamma=1.0, lam=0.95)
    assert batch["slot_num_feats"].dtype == torch.float16
    assert batch["places"].shape == batch["actions"].shape and int(batch["places"].max()) <= 7
    opt = torch.optim.Adam(model.parameters(), lr=1e-3)
    metrics = ppo_update(model, opt, batch, epochs=1, minibatch_size=128)
    assert np.isfinite(metrics["value_loss"]) and metrics["value_loss"] > 0


def test_opponent_set_samples_history_and_keeps_the_latest() -> None:
    from seaglass_rl.train_lobby import opponent_set

    pool = [f"s{i:02d}" for i in range(20)]
    rng = random.Random(0)
    seen_old = set()
    for _ in range(30):
        got = opponent_set(pool, 4, 4, rng)
        assert len(got) == len(set(got)) == 8
        assert set(pool[-4:]) <= set(got)
        seen_old |= set(got) - set(pool[-4:])
    assert len(seen_old) > 4  # the history draw varies
    assert opponent_set(pool, 10, 0, rng) == pool[-10:]
    assert opponent_set(pool[:3], 4, 4, rng) == pool[:3]
    assert opponent_set([], 4, 4, rng) == []


def test_league_opponents_are_bots_without_a_pool_and_snapshots_from_the_set() -> None:
    from seaglass_rl.train_lobby import make_specs, opponent_set

    config = LobbyConfig()
    bots = ["9kgamerbot", "9kgamerbot-tempo"]
    rng = random.Random(1)
    specs = make_specs(config, 20, 4, [], bots, 0.3, rng, seed=0)
    others = [c for s in specs for c in s.controllers if c != "learner"]
    assert others and set(others) == set(bots)
    pool = [f"s{i:02d}" for i in range(20)]
    opponents = opponent_set(pool, 4, 4, rng)
    specs = make_specs(config, 20, 4, opponents, bots, 0.3, rng, seed=0)
    others = {c for s in specs for c in s.controllers if c != "learner"}
    assert others <= set(bots) | set(opponents) and len(others - set(bots)) <= 8
    assert others & set(bots) and others & set(opponents)


def test_train_lobby_guards_and_aliases(tmp_path) -> None:
    from seaglass_rl import train_lobby

    with pytest.raises(SystemExit, match="placement critic"):
        train_lobby.main(["--run-dir", str(tmp_path / "a"), "--shaping", "0.3", "--workers", "0", "--device", "cpu"])
    with pytest.raises(SystemExit, match="unknown bots"):
        train_lobby.main(["--run-dir", str(tmp_path / "b"), "--bots", "nope", "--device", "cpu"])


def test_recorded_bots_also_end_the_end_of_turn_step() -> None:
    from seaglass_rl.actions import END_TURN_INDEX
    from seaglass_rl.features import PHASE_OFFSET

    config = LobbyConfig()
    (res,) = play_games(config, [GameSpec(seed=4, controllers=["heuristic"] * 8)], {}, record=("heuristic",))
    for tr in res.trajectories:
        arranging = tr["player_vec"][:, PHASE_OFFSET] > 0.5
        assert arranging.any()
        # Each recorded end-of-turn step is one End Turn, right after the shopping's End Turn.
        idx = np.nonzero(arranging)[0]
        assert (tr["actions"][idx] == END_TURN_INDEX).all()
        assert (tr["actions"][idx - 1] == END_TURN_INDEX).all() and not tr["turn_end"][idx - 1].any()
        assert tr["action_mask"][idx, END_TURN_INDEX].all()
    # Without the step, nothing is recorded in it.
    (off,) = play_games(
        config, [GameSpec(seed=4, controllers=["heuristic"] * 8)], {}, PlayOptions(positioning_phase=False), record=("heuristic",)
    )
    assert all(not (tr["player_vec"][:, PHASE_OFFSET] > 0.5).any() for tr in off.trajectories)


@pytest.mark.parametrize("arch", ["mlp", "transformer"])
def test_reset_critic_head_gives_a_uniform_place(arch: str) -> None:
    from seaglass_rl.pretrain import reset_critic_head

    m = BattlegroundsMLP(seed=0, value_places=8) if arch == "mlp" else EntityTransformer(seed=0, value_places=8)
    reset_critic_head(m)
    config = LobbyConfig()
    (res,) = play_games(config, [GameSpec(seed=2, controllers=["heuristic"] * 8)], {}, record=("heuristic",))
    tr = res.trajectories[0]
    with torch.no_grad():
        _, value, logits = m(
            torch.from_numpy(tr["player_vec"][:4]),
            torch.from_numpy(tr["slot_card_ids"][:4].astype(np.int64)),
            torch.from_numpy(tr["slot_num_feats"][:4].astype(np.float32)),
            torch.from_numpy(tr["action_mask"][:4]),
            return_value_logits=True,
        )
    assert torch.allclose(logits, torch.zeros_like(logits)) and torch.allclose(value, torch.zeros_like(value), atol=1e-6)
