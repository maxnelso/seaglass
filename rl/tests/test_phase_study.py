"""`LobbyGame.reseed` and the forked-rollout study (`rl/scripts/phase_study.py`)."""

from __future__ import annotations

import copy
import importlib.util
from pathlib import Path

import pytest

from seaglass_rl.play import LobbyConfig, _bot

_SPEC = importlib.util.spec_from_file_location(
    "phase_study", Path(__file__).resolve().parents[1] / "scripts" / "phase_study.py"
)
phase_study = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(phase_study)

FIELD = ("9kgamerbot", "9kgamerbot-tempo") * 4


def _lobby_at_turn(seed: int, turn: int):
    """A bot lobby at the start of `turn`, with its bots."""
    game = LobbyConfig().make(seed)
    bots = [_bot(FIELD[p], seed * 16 + p) for p in range(game.num_players)]
    game.begin_phase()
    while not (game.phase == "turn" and game.turn == turn):
        phase_study.act_phase(game, bots)
        game.end_phase()
        game.begin_phase()
    return game, bots


def test_a_copy_replays_the_same_game_until_it_is_reseeded():
    game, bots = _lobby_at_turn(seed=5, turn=3)
    shops = [[u.card_id for u in game.observe(p).shop] for p in range(8)]

    def play(reseed: int | None) -> list[int]:
        fork = game.copy()
        if reseed is not None:
            fork.reseed(reseed)
            # Nothing already dealt changes.
            assert [[u.card_id for u in fork.observe(p).shop] for p in range(8)] == shops
        return phase_study.play_out(fork, copy.deepcopy(bots))

    same = play(None)
    assert play(None) == same
    assert play(101) == play(101)
    assert sorted(same) == list(range(1, 9))
    # Eight other seeds do not all deal the game the copy would have had.
    assert any(play(200 + k) != same for k in range(8))


def test_study_game_forks_every_turn():
    out = phase_study.study_game((9, FIELD, 3, True))
    assert sorted(out["placements"]) == list(range(1, 9))
    assert [s["turn"] for s in out["snapshots"]] == list(range(1, out["turns"] + 1))
    for snap in out["snapshots"]:
        assert len(snap["places"]) == 3 and len(snap["skip_places"]) == 3
        assert snap["alive"][snap["skip_unit"]]
        for places in snap["places"]:
            assert sorted(places) == list(range(1, 9))
            # A unit that is out keeps its place in every fork.
            for u, alive in enumerate(snap["alive"]):
                if not alive:
                    assert places[u] == out["placements"][u]


def test_position_stats_split_noise_from_signal():
    # Two positions, one living seat each: places 1,1,3,3 and 6,6,8,8.
    snaps = [
        {"alive": [True], "places": [[1], [1], [3], [3]]},
        {"alive": [True], "places": [[6], [6], [8], [8]]},
    ]
    stats = phase_study.position_stats(snaps)
    assert stats["seats"] == 2
    assert stats["noise_sd"] == pytest.approx((4 / 3) ** 0.5)
    # Pooled variance 7.25, less the within-position 4/3.
    assert stats["signal_sd"] == pytest.approx((7.25 - 4 / 3) ** 0.5)
    assert phase_study.position_stats([]) == {}
