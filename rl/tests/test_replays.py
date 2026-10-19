"""Tests for recorded matches: `seaglass.Match`/`Timeline` and `seaglass_rl.replays`."""

from __future__ import annotations

import random

import numpy as np
import pytest
import seaglass

from seaglass_rl import BattlegroundsEnv, BattlegroundsMLP, encode_observation, replays
from seaglass_rl.bots import finish_turn
from seaglass_rl.train_ppo import collect_selfplay_rollouts, evaluate_policy

CHOOSE_FIRST = seaglass.TavernAction.choose_discover(0)


def _unit_key(u: seaglass.Unit) -> tuple:
    return (
        u.card_id,
        u.name,
        u.attack,
        u.health,
        u.tavern_tier,
        u.is_golden,
        u.is_spell,
        u.taunt,
        u.divine_shield,
        u.windfury,
        u.reborn,
        u.venomous,
    )


def _snapshot(obs: seaglass.TavernObservation) -> tuple:
    discover = obs.discover_pending
    return (
        obs.turn,
        obs.health,
        obs.armor,
        obs.gold,
        obs.max_gold,
        obs.tavern_tier,
        obs.upgrade_cost,
        obs.free_refreshes,
        tuple(map(_unit_key, obs.board)),
        tuple(map(_unit_key, obs.hand)),
        tuple(map(_unit_key, obs.shop)),
        None if discover is None else tuple(map(_unit_key, discover)),
    )


def _assert_same(old: list, new: list) -> None:
    for o, n in zip(old, new):
        obs_o, obs_n = o.observe(), n.observe()
        assert _snapshot(obs_o) == _snapshot(obs_n)
        for x, y in zip(encode_observation(obs_o), encode_observation(obs_n)):
            np.testing.assert_array_equal(x, y)


@pytest.mark.parametrize("seed", [3, 17, 2024])
def test_match_plays_like_two_tavern_games(seed: int) -> None:
    """The same calls give the same games through a `Match` as through two `TavernGame`s."""
    old = [
        seaglass.TavernGame(seed=seed, catalog="full", deity="cthun"),
        seaglass.TavernGame(seed=seed + 1, catalog="full", deity="yshaarj"),
    ]
    match = seaglass.Match(
        players=[
            {"name": "a", "seed": seed, "deity": "cthun"},
            {"name": "b", "seed": seed + 1, "deity": "yshaarj"},
        ]
    )
    new = match.players()
    rng = random.Random(seed)

    for _ in range(12):
        for o, n in zip(old, new):
            o.start_turn()
            n.start_turn()
        _assert_same(old, new)
        for o, n in zip(old, new):
            for _ in range(10):
                legal = o.legal_actions()
                assert list(map(repr, legal)) == list(map(repr, n.legal_actions()))
                act = rng.choice(legal)
                assert o.step(act) == n.step(act)
                _assert_same(old, new)
                if act.kind == "end_turn":
                    break
            else:
                while o.observe().discover_pending is not None:
                    o.step(CHOOSE_FIRST)
                    n.step(CHOOSE_FIRST)
                o.end_turn()
                n.end_turn()
            _assert_same(old, new)
        res_o = old[0].resolve_combat_with(old[1])
        res_n = new[0].resolve_combat_with(new[1])
        assert (res_o.outcome, res_o.hero_damage) == (res_n.outcome, res_n.hero_damage)
        assert list(map(_unit_key, res_o.survivors_a)) == list(map(_unit_key, res_n.survivors_a))
        assert list(map(_unit_key, res_o.survivors_b)) == list(map(_unit_key, res_n.survivors_b))
        _assert_same(old, new)
        if any(o.observe().health <= 0 for o in old):
            break

    match.finish()
    timeline = match.timeline()
    assert timeline.is_faithful
    assert timeline[-1].kind == "end"
    # Every frame holds both players' states after its call.
    for o, obs in zip(old, timeline[-1].players):
        assert _snapshot(o.observe()) == _snapshot(obs)


def test_env_episode_replays_into_the_same_training_rows() -> None:
    """An env episode is recorded faithfully, and `to_dataset` rebuilds exactly what the policy saw."""
    env = BattlegroundsEnv(opponent="heuristic", seed=321)
    model = BattlegroundsMLP(seed=1)
    obs, mask = env.reset()
    live = []
    done = False
    info: dict = {}
    while not done:
        act, lp, val = model.act(obs, mask)
        live.append((obs, mask, act, lp, val))
        obs, _, done, mask, info = env.step(act, note={"value": val, "logp": lp})

    timeline = env.match.timeline()
    assert timeline.is_faithful
    assert timeline.engine_mismatch is None
    extras = {"armor": 5, "shop_spells": True, "hero": None, "trinkets": True, "dark_gifts": True}
    assert timeline.header["players"] == [
        {"name": "agent", "seed": 321, "deity": "cthun", **extras},
        {"name": "heuristic", "seed": 322, "deity": "yshaarj", **extras},
    ]
    assert timeline[-1].kind == "end"
    assert timeline[-1].winner == {"agent": 0, "opponent": 1, "draw": None}[info["winner"]]

    ds = replays.to_dataset(timeline, player="agent")
    chosen = np.flatnonzero(~ds["forced"])
    assert len(chosen) == len(live)
    for row, (o, m, a, lp, val) in zip(chosen, live):
        np.testing.assert_array_equal(ds["player_vec"][row], o[0])
        np.testing.assert_array_equal(ds["slot_card_ids"][row], o[1])
        np.testing.assert_array_equal(ds["slot_num_feats"][row], o[2])
        np.testing.assert_array_equal(ds["action_mask"][row], m)
        assert ds["actions"][row] == a
        assert ds["value"][row] == np.float32(val)
        assert ds["logp"][row] == np.float32(lp)
    assert np.isnan(ds["value"][ds["forced"]]).all()
    assert (ds["player"] == 0).all()
    assert (ds["outcome"] == replays.match_outcome(timeline)[0]).all()


def test_notes_round_trip(tmp_path) -> None:
    match = seaglass.Match(players=[{"name": "p0", "seed": 5}, {"name": "p1", "seed": 6}])
    p = match.player(0)
    p.start_turn()
    buy = next(a for a in p.legal_actions() if a.kind == "buy")
    refresh = seaglass.TavernAction.refresh()
    note = {"value": 0.25, "logp": -0.5, "candidates": [(buy, 0.75), (refresh, 0.25)], "entropy": 1.25}
    p.step(buy, note=note)
    p.end_turn(note={"forced": True})

    records = match.num_records
    with pytest.raises(TypeError, match='note entry "value" must be a number'):
        p.step(buy, note={"value": "high"})
    with pytest.raises(ValueError):
        p.step(buy)  # The turn is over.
    assert match.num_records == records

    path = tmp_path / "nested" / "notes.replay.pb"
    match.save(path)
    timeline = replays.load(path)
    (_, p1, a1, n1), (_, p2, a2, n2) = timeline.decisions()
    assert (p1, repr(a1), p2, a2.kind) == (0, repr(buy), 0, "end_turn")
    assert {k: v for k, v in n1.items() if k != "candidates"} == {
        "value": 0.25,
        "logp": -0.5,
        "forced": False,
        "entropy": 1.25,
    }
    assert [(repr(a), prob) for a, prob in n1["candidates"]] == [(repr(buy), 0.75), (repr(refresh), 0.25)]
    assert n2 == {"forced": True}


def test_timeline_navigation() -> None:
    env = BattlegroundsEnv(opponent="random", seed=99)
    model = BattlegroundsMLP(seed=2)
    obs, mask = env.reset()
    done = False
    while not done:
        act, _, _ = model.act(obs, mask)
        obs, _, done, mask, _ = env.step(act)
    timeline = seaglass.Timeline.from_bytes(env.match.replay_bytes())

    assert len(timeline) == env.match.num_records + 1
    assert timeline[0].kind == "start" and timeline[0].record is None
    assert timeline[-1].index == len(timeline) - 1
    kinds = {f.kind for f in timeline}
    assert kinds == {
        "start",
        "set_pairs",
        "start_turn",
        "deliver_gifts",
        "act",
        "combat",
        "record_deaths",
        "end",
    }
    t2 = timeline.turn_start(2)
    assert timeline[t2].turn == 2 and timeline[t2 - 1].turn == 1
    combat = next(f for f in timeline if f.kind == "combat")
    assert combat.combatants == (0, 1) and combat.battle is not None and combat.num_events > 0
    with pytest.raises(IndexError):
        timeline[len(timeline)]
    with pytest.raises(IndexError):
        timeline.player_at(0, 2)


def test_trainer_records_selfplay_and_eval_games(tmp_path) -> None:
    model = BattlegroundsMLP(seed=3)
    batch = collect_selfplay_rollouts(model, games_per_iter=2, seed_base=11, game_counter_start=0, max_turns=5)
    timeline = batch["last_match"].timeline()
    assert timeline.is_faithful
    assert timeline[-1].kind == "end"
    assert timeline.header["meta"]["source"] == "selfplay"
    assert [p["seed"] for p in timeline.header["players"]] == [13, 14]
    notes = [n for _, _, _, n in timeline.decisions()]
    assert all(n is not None for n in notes)
    assert all("value" in n and "logp" in n for n in notes if not n["forced"])

    evaluate_policy(model, opponent="random", episodes=2, seed=5, replay_dir=str(tmp_path), replay_tag="t")
    files = sorted(tmp_path.glob("*.replay.pb"))
    assert [f.name for f in files] == ["t_random_000.replay.pb", "t_random_001.replay.pb"]
    ds = replays.to_dataset(files, player="agent")
    assert set(ds["source"]) == {0, 1}
    assert len(ds["actions"]) == ds["player_vec"].shape[0] == ds["action_mask"].shape[0]
    assert ds["action_mask"][np.arange(len(ds["actions"])), ds["actions"]].all()
    assert replays.load(files[0]).header["meta"]["eval"] == "t"


def test_match_players_take_heroes_and_the_lobby_is_recorded() -> None:
    match = seaglass.Match(
        players=[
            {"name": "a", "seed": 1, "hero": "Patchwerk", "trinkets": False},
            {"name": "b", "seed": 2, "dark_gifts": False},
        ]
    )
    a, b = match.player(0), match.player(1)
    assert a.observe().health == seaglass.TavernGame(seed=1, hero="Patchwerk").observe().health
    with pytest.raises(ValueError):
        seaglass.Match(players=[{"name": "x", "hero": "Nobody At All"}])
    with pytest.raises(ValueError):
        match.set_pairs([(0, 0)])
    match.set_pairs([(0, 1)])
    a.start_turn()
    b.start_turn()
    match.deliver_gifts()
    finish_turn(a)
    finish_turn(b)
    match.combat(0, 1)
    assert match.record_deaths() == [True, True]
    match.finish()
    timeline = match.timeline()
    assert timeline.is_faithful
    assert [f.kind for f in timeline][:5] == ["start", "set_pairs", "start_turn", "start_turn", "deliver_gifts"]
    assert timeline.header["players"][0]["hero"] == "Patchwerk"
    assert timeline.header["players"][1]["hero"] is None
    assert timeline.header["players"][0]["trinkets"] is False
