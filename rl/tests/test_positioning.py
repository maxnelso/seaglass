"""Positioning by search (`positioning.py`) and its hook in `play_games`."""

from __future__ import annotations

import random

import pytest
import seaglass
import torch

from seaglass_rl import ladder
from seaglass_rl.actions import ACTION_KINDS, ACTION_KIND_IDS, action_from_index
from seaglass_rl.bots import HeuristicBot
from seaglass_rl.model import BattlegroundsMLP
from seaglass_rl.play import GameSpec, LobbyConfig, PlayOptions, Seat, play_games
from seaglass_rl.positioning import PositionSpec, best_order, moves_to, opponent_boards, score_order
from seaglass_rl.probe import ProbeSpec

SMALL = ProbeSpec(lobbies=4, per_turn=8, min_boards=4)


def _key(units) -> list[tuple]:
    return [(u.card_id, u.attack, u.health) for u in units]


def _lobby_with_board(min_board: int = 4):
    """A lobby of HeuristicBots, stopped at a player's turn with `min_board`+ minions."""
    game = LobbyConfig().make(5)
    bot = HeuristicBot()
    for _ in range(40):
        game.begin_phase()
        if game.phase == "turn":
            for p in game.active_players():
                if len(Seat(game, p).observe().board) >= min_board:
                    return game, p
            for p in game.active_players():
                seat = Seat(game, p)
                for _ in range(25):
                    if game.is_done(p):
                        break
                    seat.step(action_from_index(bot.act(seat)))
        game.end_phase()
    raise AssertionError("no board of 4+ minions")


def test_moves_to_matches_the_engines_reposition() -> None:
    game, p = _lobby_with_board()
    board = list(Seat(game, p).observe().board)
    n = len(board)
    rng = random.Random(1)
    for _ in range(6):
        perm = list(range(n))
        rng.shuffle(perm)
        moves = moves_to(perm)
        assert len(moves) <= n - 1
        g = game.copy()
        for f, t in moves:
            g.step(p, seaglass.TavernAction.reposition(f, t))
        assert _key(Seat(g, p).observe().board) == _key([board[i] for i in perm])
    assert moves_to(list(range(n))) == []


def test_best_order_is_a_deterministic_permutation_that_does_not_lose_to_the_identity() -> None:
    game, p = _lobby_with_board()
    obs = Seat(game, p).observe()
    board, n = list(obs.board), len(obs.board)
    spec = PositionSpec(library=SMALL, max_evals=10)
    opps = opponent_boards(game, p, LobbyConfig(), spec)
    assert opps
    perm, evals = best_order(board, opps, obs.tavern_tier, 11, spec)
    assert sorted(perm) == list(range(n))
    assert 0 < evals <= spec.max_evals
    assert (perm, evals) == best_order(board, opps, obs.tavern_tier, 11, spec)
    after = score_order([board[i] for i in perm], opps, obs.tavern_tier, 11, spec.sims)
    assert after >= score_order(board, opps, obs.tavern_tier, 11, spec.sims)
    assert best_order(board[:1], opps, 1, 0, spec) == ([0], 0)
    assert best_order(board, [], 1, 0, spec) == (list(range(n)), 0)


def test_opponent_boards() -> None:
    config = LobbyConfig()
    game = config.make(2)
    game.begin_phase()
    assert opponent_boards(game, 0, config, PositionSpec(library=None)) == []  # nothing seen yet
    game, p = _lobby_with_board()
    seen_only = opponent_boards(game, p, config, PositionSpec(library=None))
    assert len(seen_only) <= 4
    full = opponent_boards(game, p, config, PositionSpec(library=SMALL, max_opponents=3))
    assert 0 < len(full) <= 3
    assert len(full) >= len(seen_only[:3])


def _play(options: PlayOptions, record=("learner",)):
    torch.manual_seed(0)
    model = BattlegroundsMLP(seed=0, kind_prior=True)
    ctrl = ["learner"] * 4 + ["heuristic"] * 4
    specs = [GameSpec(seed=3 + i, controllers=ctrl) for i in range(3)]
    return play_games(LobbyConfig(), specs, {"learner": model}, options, record=record)


def test_auto_position_orders_model_seats_only_and_records_nothing() -> None:
    opts = PlayOptions(auto_position=PositionSpec(library=SMALL), turn_stats=True, allow_reposition=False)
    res = _play(opts)
    # Units 0-3 are the model's, 4-7 the bots'.
    def count(kind: str, units: range) -> int:
        return sum(t.actions.get(kind, 0) for r in res for u in units for t in r.turn_stats[u])

    assert count("auto_position", range(4)) > 0 and count("auto_position", range(4, 8)) == 0
    assert count("reposition", range(8)) == 0


def test_recorded_trajectories_hold_no_auto_moves() -> None:
    opts = PlayOptions(auto_position=PositionSpec(library=SMALL), allow_reposition=False)
    res = _play(opts)
    assert res[0].trajectories
    for t in [t for r in res for t in r.trajectories]:
        assert len(t["actions"])
        kinds = {ACTION_KINDS[ACTION_KIND_IDS[a]] for a in t["actions"]}
        assert "reposition" not in kinds


def test_auto_position_is_solo_only() -> None:
    model = BattlegroundsMLP(seed=0, kind_prior=True)
    with pytest.raises(ValueError):
        play_games(
            LobbyConfig(duos=True),
            [GameSpec(seed=1, controllers=["learner"] * 4)],
            {"learner": model},
            PlayOptions(auto_position=PositionSpec()),
        )


def test_ladder_reads_auto_position_from_the_checkpoint(tmp_path) -> None:
    from seaglass_rl import checkpoint

    model = BattlegroundsMLP(seed=0, kind_prior=True)
    for flag in (True, False):
        path = str(tmp_path / f"c{flag}.pt")
        checkpoint.save(path, model, iteration=0, args={"auto_position": flag})
        assert (ladder.checkpoint_options(path).auto_position is not None) == flag
