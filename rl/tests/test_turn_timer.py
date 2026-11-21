"""The lobby's simulated turn clock (`engine/docs/lobby_game.md` §2) and the play loop under it."""

from __future__ import annotations

import seaglass

from seaglass_rl.model import BattlegroundsMLP
from seaglass_rl.play import GameSpec, LobbyConfig, PlayOptions, play_games


def _toggles_until_done(game: seaglass.LobbyGame, p: int, limit: int = 10_000) -> int:
    steps = 0
    while steps < limit:
        steps += 1
        if game.step(p, seaglass.TavernAction.toggle_freeze()):
            break
    return steps


def test_a_turn_ends_when_the_clock_runs_out() -> None:
    game = seaglass.LobbyGame(players=2, heroes=False, seed=1)
    game.begin_phase()
    assert game.turn_seconds == 45.0
    assert game.turn_timer and game.action_ms == 250
    assert _toggles_until_done(game, 0) == 180
    assert game.is_done(0)
    assert game.time_left(0) == 0.0
    assert game.time_left(1) == 45.0
    assert not game.is_done(1)


def test_the_timer_can_be_off() -> None:
    game = seaglass.LobbyGame(players=2, heroes=False, seed=1, turn_timer=False)
    game.begin_phase()
    for _ in range(500):
        assert not game.step(0, seaglass.TavernAction.toggle_freeze())
    assert not game.is_done(0)
    assert game.time_left(0) is None
    assert game.turn_seconds is None


def test_an_action_can_take_longer() -> None:
    game = seaglass.LobbyGame(players=2, heroes=False, seed=1, action_ms=500)
    game.begin_phase()
    assert _toggles_until_done(game, 0) == 90


def test_turn_seconds_by_turn() -> None:
    assert seaglass.turn_seconds(1) == 45
    assert seaglass.turn_seconds(0) == 45
    assert seaglass.turn_seconds(2, duos=True) == 55
    assert seaglass.turn_seconds(99) == 90


def test_games_play_to_the_end_without_a_step_cap() -> None:
    config = LobbyConfig(players=8)
    specs = [GameSpec(seed=s, controllers=["learner"] * 2 + ["9kgamerbot"] * 6) for s in range(2)]
    results = play_games(
        config, specs, {"learner": BattlegroundsMLP(seed=1)}, PlayOptions(), record=("learner",)
    )
    assert len(results) == 2
    for r in results:
        assert r.trajectories and all(len(t["rewards"]) > 0 for t in r.trajectories)
