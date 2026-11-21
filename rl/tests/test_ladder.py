"""The ladder (`ladder.py`), turn stats in `play.py` and the board probe (`probe.py`)."""

from __future__ import annotations

import json
from collections import Counter

import pytest

from seaglass_rl import ladder, probe
from seaglass_rl.model import BattlegroundsMLP
from seaglass_rl.play import GameSpec, LobbyConfig, Player, PlayOptions, play_games
from seaglass_rl.probe import ProbeSpec

SMALL_PROBE = ProbeSpec(lobbies=4, per_turn=8, min_boards=4)


def test_ladder_specs_are_deterministic_and_do_not_depend_on_the_agent() -> None:
    lad = ladder.LADDERS["fast"]
    a = ladder.ladder_specs(lad, 10, 77, "agent", probe=True)
    assert a == ladder.ladder_specs(lad, 10, 77, "agent", probe=True)
    b = ladder.ladder_specs(lad, 10, 77, "heuristic", probe=False)
    for (sa, ua), (sb, ub) in zip(a, b):
        assert len(ua) == 2 and ua == ub
        assert sa.probe_units == ua and sb.probe_units == ()
        assert Counter(c for i, c in enumerate(sa.controllers) if i not in ua) == Counter(lad.anchors)
        assert [c for i, c in enumerate(sa.controllers) if i not in ua] == [
            c for i, c in enumerate(sb.controllers) if i not in ub
        ]
        assert all(sa.controllers[u] == "agent" for u in ua)
    assert len({tuple(s.controllers) for s, _ in a}) > 1


def test_turn_stats() -> None:
    config = LobbyConfig()
    specs = [GameSpec(seed=3, controllers=["heuristic"] * 8)]
    (res,) = play_games(config, specs, {}, PlayOptions(turn_stats=True), record=())
    assert len(res.turn_stats) == 8
    for stats in res.turn_stats:
        assert stats
        turns = [s.turn for s in stats]
        assert turns[0] == 1 and all(b > a for a, b in zip(turns, turns[1:]))
        for s in stats:
            assert 1 <= s.tier <= 7 and s.gold_left >= 0 and s.board >= 0
            assert all(isinstance(n, int) and n >= 0 for n in s.actions.values())
            assert s.probe is None
    (off,) = play_games(config, specs, {}, record=())
    assert off.turn_stats == [] and off.placements == res.placements
    with pytest.raises(ValueError):
        play_games(LobbyConfig(duos=True), [GameSpec(seed=1, controllers=["heuristic"] * 4)], {}, PlayOptions(turn_stats=True))


def test_probe_library_and_score() -> None:
    config = LobbyConfig()
    lib = probe.build_library(SMALL_PROBE, config)
    assert lib
    for refs in lib.values():
        assert SMALL_PROBE.min_boards <= len(refs) <= SMALL_PROBE.per_turn
    again = probe.build_library(SMALL_PROBE, config)
    sig = lambda x: {t: [(tier, len(b)) for b, tier in refs] for t, refs in x.items()}  # noqa: E731
    assert sig(lib) == sig(again)
    assert probe.library_summary(lib) == {t: len(r) for t, r in sorted(lib.items())}
    turn = sorted(lib)[len(lib) // 2]
    board, tier = lib[turn][0]
    score = probe.score_board(lib, board, tier, turn, seed=1, sims=2)
    assert isinstance(score, float) and 0.0 <= score <= 1.0
    assert probe.score_board(lib, board, tier, 999, seed=1, sims=2) is None
    empty = probe.score_board(lib, [], 1, turn, seed=1, sims=2)
    own = [probe.score_board(lib, b, t, turn, seed=5, sims=2) for b, t in lib[turn]]
    assert 0.0 <= empty <= sum(own) / len(own)


def test_rate_a_bot() -> None:
    kwargs = dict(games=4, probe=SMALL_PROBE, player=Player(0))
    res = ladder.rate("heuristic", **kwargs)
    for key in ("meta", "agent", "anchors", "by_turn", "behavior", "lobbies"):
        assert key in res
    assert 1 <= res["agent"]["avg_place"] <= 8
    assert res["agent"]["seat_games"] == 8
    assert set(res["by_turn"]) == {"agent", *ladder.LADDERS["fast"].anchors}
    assert set(res["anchors"]) == set(ladder.LADDERS["fast"].anchors)
    probes = [e["probe"] for e in res["by_turn"]["agent"].values() if "probe" in e]
    assert probes and all(0.0 <= p <= 1.0 for p in probes)
    assert res["meta"]["probe_library"]
    json.dumps(res)
    again = ladder.rate("heuristic", **kwargs)
    assert again["lobbies"] == res["lobbies"] and again["agent"]["avg_place"] == res["agent"]["avg_place"]
    assert "avg place" in ladder.format_report(res) and "probe" in ladder.format_summary(res)


def test_rate_a_model() -> None:
    m = BattlegroundsMLP(seed=0, kind_prior=True)
    res = ladder.rate((dict(m.config), m.state_dict()), games=2, probe=None, player=Player(0))
    assert res["agent"]["seat_games"] == 4
    assert res["meta"]["agent"] == "<state_dict>"
    assert all("probe" not in e for e in res["by_turn"]["agent"].values())
    assert "avg place" in ladder.format_report(res)


def test_compare() -> None:
    kw = dict(probe=None, player=Player(0))
    a = ladder.rate("heuristic", games=4, **kw)
    b = ladder.rate("9kgamerbot", games=4, **kw)
    c = ladder.rate("heuristic", games=4, seed=5_100_000, **kw)
    same = ladder.compare(a, b)
    assert same["paired"] is not None and same["paired"]["lobbies"] == 4
    assert abs(same["diff"] - (a["agent"]["avg_place"] - b["agent"]["avg_place"])) < 1e-12
    other = ladder.compare(a, c)
    assert other["paired"] is None and isinstance(other["agree"], bool)
    loaded = [json.loads(json.dumps(x)) for x in (a, b, c)]
    assert ladder.compare(loaded[0], loaded[1])["paired"] is not None
    assert ladder.compare(loaded[0], loaded[2])["paired"] is None
    assert "a - b" in ladder.format_compare(other)
    assert "avg place" in ladder.format_report(loaded[0])
    with pytest.raises(ValueError):
        ladder.compare(a, {**a, "meta": {**a["meta"], "ladder": "slow"}})


def test_checkpoints_are_rated_under_their_training_rules(tmp_path) -> None:
    from seaglass_rl import checkpoint

    m = BattlegroundsMLP(seed=0, kind_prior=True)
    path = tmp_path / "m.pt"
    checkpoint.save(path, m, iteration=1, args={"no_reposition": True, "max_steps_per_turn": 25})
    opts = ladder.checkpoint_options(str(path))
    assert not opts.allow_reposition and opts.max_steps_per_turn == 25 and opts.positioning_phase
    assert ladder.checkpoint_options("heuristic") == PlayOptions()
