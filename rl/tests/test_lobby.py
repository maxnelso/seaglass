"""Full lobbies (`seaglass.LobbyGame`), playing them (`play.py`), PPO's per-turn GAE (`ppo.py`),
checkpoints and evaluation."""

from __future__ import annotations

import copy

import numpy as np
import pytest
import seaglass
import torch

from seaglass_rl import checkpoint
from seaglass_rl.bots import HeuristicBot
from seaglass_rl.evaluate import evaluate
from seaglass_rl.features import (
    DUOS_OFFSET,
    HERO_OFFSET,
    LOBBY_OFFSET,
    NUM_SLOTS,
    OPPONENT_BOARD_OFFSET,
    OPPONENTS_OFFSET,
    PLAYER_DIM,
    TEAMMATE_OFFSET,
    encode_observation,
)
from seaglass_rl.actions import action_from_index
from seaglass_rl.model import BattlegroundsMLP
from seaglass_rl.play import GameSpec, LobbyConfig, PlayOptions, Seat, placement_reward, play_games
from seaglass_rl.ppo import build_batch, gae, ppo_update


def _bots(config: LobbyConfig, seed: int) -> list:
    return play_games(config, [GameSpec(seed=seed, controllers=["heuristic"] * config.units)], {})


@pytest.mark.parametrize("duos", [False, True])
def test_a_lobby_plays_to_the_end_and_places_every_unit(duos: bool) -> None:
    config = LobbyConfig(duos=duos)
    (res,) = _bots(config, seed=3)
    assert sorted(res.placements) == list(range(1, config.units + 1))
    assert 2 <= res.turns <= config.max_turns
    (again,) = _bots(config, seed=3)
    assert again.placements == res.placements and again.turns == res.turns


@pytest.mark.parametrize("duos", [False, True])
def test_played_lobbies_replay_faithfully(duos: bool, tmp_path) -> None:
    config = LobbyConfig(duos=duos)
    model = BattlegroundsMLP(seed=2, kind_prior=True)
    controllers = ["learner", "heuristic", "random", "heuristic"][: config.units] + ["heuristic"] * (config.units - 4)
    (res,) = play_games(
        config, [GameSpec(seed=8, controllers=controllers)], {"learner": model}, PlayOptions(keep_replays=True)
    )
    path = tmp_path / "game.replay.pb"
    res.save_replay(path)
    timeline = seaglass.Timeline.load(str(path))
    assert timeline.is_faithful
    end = timeline[-1]
    assert end.kind == "end" and end.placements == res.placements
    kinds = {f.kind for f in timeline}
    assert ("duos_combat" if duos else "combat") in kinds
    # The learner's actions carry its value and log-probability.
    notes = [f.note for f in timeline if f.kind == "act" and f.note and "logp" in f.note]
    assert notes


def test_hero_offers_are_distinct_across_the_lobby() -> None:
    offers = seaglass.draw_hero_offers(8, 2, False, True, 5)
    assert all(len(o) == 2 for o in offers)
    flat = [h for o in offers for h in o]
    assert len(set(flat)) == len(flat)
    game = LobbyConfig().make(5)
    assert game.phase == "hero"
    assert [len(game.observe(p).hero_options) for p in range(8)] == [2] * 8
    # Nobody acts before the phase begins.
    assert game.legal_actions(0) == [] and game.active_players() == []
    game.begin_phase()
    assert game.active_players() == list(range(8))
    assert {a.kind for a in game.legal_actions(0)} == {"choose_hero"}


def test_a_game_copy_is_independent() -> None:
    game = seaglass.TavernGame(seed=1)
    game.start_turn()
    other = game.copy()
    other.step(seaglass.TavernAction.buy(0))
    assert len(game.observe().hand) == 0 and len(other.observe().hand) == 1
    assert len(copy.deepcopy(other).observe().hand) == 1
    team = seaglass.DuosTavernGame(seed=1)
    team.start_turn()
    assert team.copy().observe(0).turn == 1


def _play_turns(game: seaglass.LobbyGame, turns: int) -> None:
    """Play `turns` phases with HeuristicBots (`end_phase` forces whatever is left)."""
    bot = HeuristicBot()
    for _ in range(turns):
        game.begin_phase()
        if game.phase == "turn":
            for p in game.active_players():
                seat = Seat(game, p)
                for _ in range(25):
                    if game.is_done(p):
                        break
                    seat.step(action_from_index(bot.act(seat)))
        game.end_phase()


@pytest.mark.parametrize("duos", [False, True])
def test_the_lobby_context_puts_the_next_opponents_first(duos: bool) -> None:
    game = LobbyConfig(duos=duos).make(9)
    _play_turns(game, 3)  # the hero phase, then turns 1 and 2
    game.begin_phase()
    ctx = game.context(0)
    assert len(ctx.opponents) == (6 if duos else 7)
    assert ctx.alive_units == game.num_units
    nxt = [o for o in ctx.opponents if o.is_next]
    assert [o.player for o in ctx.opponents[: len(nxt)]] == [o.player for o in nxt]
    assert len(nxt) == (2 if duos else 1)
    if duos:
        # The opposing player in this seat first.
        assert nxt[0].player % 2 == 0 and nxt[1].player == nxt[0].player + 1
    # Opponents are never the player or their teammate.
    assert all(o.player not in ((0, 1) if duos else (0,)) for o in ctx.opponents)


@pytest.mark.parametrize("duos", [False, True])
def test_the_encoding_fills_the_lobby_and_team_features(duos: bool) -> None:
    game = LobbyConfig(duos=duos).make(11)
    plain = encode_observation(game.observe(0))
    # Hero options are embedded before any lobby context.
    assert plain[1][HERO_OFFSET + 1] != 0 and plain[1][HERO_OFFSET] == 0
    _play_turns(game, 4)
    game.begin_phase()
    obs = game.observe(0)
    pv, ids, nums = encode_observation(obs, game.context(0))
    assert pv.shape == (PLAYER_DIM,) and ids.shape == (NUM_SLOTS,)
    assert pv[LOBBY_OFFSET] == 1.0
    assert pv[OPPONENTS_OFFSET] == 1.0 and pv[OPPONENTS_OFFSET + 5] == 1.0  # next opponent first
    assert ids[HERO_OFFSET] != 0  # the chosen hero
    assert (ids[OPPONENT_BOARD_OFFSET:] != 0).any()  # their last board
    assert (pv[DUOS_OFFSET] == 1.0) == duos
    assert (ids[TEAMMATE_OFFSET : OPPONENT_BOARD_OFFSET] != 0).any() == duos
    # Without a context the lobby features stay 0.
    pv0, _, _ = encode_observation(obs)
    assert not pv0[LOBBY_OFFSET:DUOS_OFFSET].any()


@pytest.mark.parametrize("duos", [False, True])
def test_recorded_rewards_sum_to_the_placement_reward(duos: bool) -> None:
    config = LobbyConfig(duos=duos)
    model = BattlegroundsMLP(seed=1)
    controllers = ["learner", "heuristic"] * (config.units // 2)
    (res,) = play_games(config, [GameSpec(seed=2, controllers=controllers)], {"learner": model})
    assert len(res.trajectories) == (4 if not duos else 4)  # 4 Solo learners, 2 teams of 2
    for tr in res.trajectories:
        assert bool(tr["done"]) and tr["turn_end"][-1]
        # No shaping: the only reward is the placement.
        assert np.count_nonzero(tr["rewards"]) == 1
        assert tr["rewards"][-1] in [placement_reward(p, config.units) for p in range(1, config.units + 1)]
        assert tr["action_mask"][np.arange(len(tr["actions"])), tr["actions"]].all()


def test_shaping_is_potential_based() -> None:
    config = LobbyConfig(players=4)
    model = BattlegroundsMLP(seed=1)
    twin = copy.deepcopy(model)
    spec = GameSpec(seed=4, controllers=["learner", "heuristic", "learner", "heuristic"])
    (plain,) = play_games(config, [spec], {"learner": model}, PlayOptions(shaping=0.0))
    (shaped,) = play_games(config, [spec], {"learner": twin}, PlayOptions(shaping=0.5, gamma=1.0))
    # Same seeds, same models: the same games. With gamma 1 the shaping telescopes to -phi(s0) = 0.
    assert shaped.placements == plain.placements
    for a, b in zip(plain.trajectories, shaped.trajectories):
        assert np.isclose(a["rewards"].sum(), b["rewards"].sum(), atol=1e-4)


def test_gae_discounts_only_between_turns() -> None:
    rewards = np.array([0.0, 0.0, 1.0], dtype=np.float32)
    values = np.zeros(3, dtype=np.float32)
    turn_end = np.array([False, True, True])
    adv, ret = gae(rewards, values, turn_end, True, gamma=0.5, lam=1.0)
    # Step 1 ends a turn: step 2's reward reaches it discounted once; step 0 shares step 1's turn.
    assert np.allclose(adv, [0.5, 0.5, 1.0]) and np.allclose(ret, adv)


def test_ppo_update_runs_on_played_games() -> None:
    config = LobbyConfig(players=4)
    model = BattlegroundsMLP(seed=3)
    specs = [GameSpec(seed=s, controllers=["learner", "heuristic", "learner", "random"]) for s in range(2)]
    results = play_games(config, specs, {"learner": model}, PlayOptions(shaping=0.1))
    batch = build_batch([t for r in results for t in r.trajectories], gamma=0.97, lam=0.95)
    opt = torch.optim.Adam(model.parameters(), lr=1e-3)
    metrics = ppo_update(model, opt, batch, epochs=2, minibatch_size=256)
    assert np.isfinite(metrics["policy_loss"]) and metrics["epochs"] >= 1


def test_checkpoints_round_trip_and_check_the_features(tmp_path) -> None:
    model = BattlegroundsMLP(hidden_dim=32, card_embed_dim=4, seed=0)
    path = tmp_path / "m.pt"
    checkpoint.save(path, model, iteration=7, args={"duos": True})
    loaded, ckpt = checkpoint.load(path)
    assert ckpt["iteration"] == 7 and ckpt["args"] == {"duos": True}
    for a, b in zip(model.state_dict().values(), loaded.state_dict().values()):
        assert torch.equal(a, b)
    ckpt["feature_spec"]["player_dim"] = 1
    torch.save(ckpt, path)
    with pytest.raises(ValueError, match="other features"):
        checkpoint.load(path)


def test_the_kind_prior_spreads_an_untrained_policy_over_kinds_alike_in_numpy_and_torch() -> None:
    game = LobbyConfig(players=2, heroes=False).make(1)
    _play_turns(game, 3)
    game.begin_phase()
    seat = Seat(game, 0)
    obs = seat.observe()
    enc = encode_observation(obs, game.context(0))
    from seaglass_rl.actions import ACTION_KIND_IDS, action_mask

    mask = action_mask(seat, allow_reposition=True, allow_freeze=True, obs=obs)
    model = BattlegroundsMLP(seed=0, kind_prior=True)
    with torch.no_grad():
        model.actor.weight.zero_()
        model.actor.bias.zero_()
        logits, value = model(*(torch.from_numpy(x[None]) for x in enc), torch.from_numpy(mask[None]))
    probs = torch.softmax(logits, -1)[0].numpy()
    kinds = set(ACTION_KIND_IDS[mask])
    per_kind = [probs[ACTION_KIND_IDS == k].sum() for k in kinds]
    assert np.allclose(per_kind, 1 / len(kinds), atol=1e-5)
    _, lps, vals = model.act_batch(*(x[None] for x in enc), mask[None], deterministic=True)
    assert np.isclose(lps[0], torch.log_softmax(logits, -1)[0].max().item(), atol=1e-4)
    assert np.isclose(vals[0], value[0].item(), atol=1e-4)


def test_behavior_cloning_learns_from_recorded_bots() -> None:
    from seaglass_rl.pretrain import collect, train

    config = LobbyConfig(players=4, duos=True)
    data = collect(config, games=2, workers=0, options=PlayOptions(shaping=0.3), seed=5)
    n = len(data["actions"])
    assert n > 50 and data["returns"].shape == (n,)
    assert data["action_mask"][np.arange(n), data["actions"]].all()
    model = BattlegroundsMLP(hidden_dim=32, card_embed_dim=4, kind_prior=True)
    stats = train(model, data, epochs=2, lr=3e-3, minibatch=256, holdout=0.2)
    assert 0.0 <= stats["accuracy"] <= 1.0 and np.isfinite(stats["value_mse"])


def test_evaluate_reports_the_average_place() -> None:
    res = evaluate("heuristic", ["random"], LobbyConfig(players=4), games=4, agent_units=1)
    assert 1.0 <= res["avg_place"] <= 4.0
    assert sum(res["places"].values()) == 4
