"""Board order by search, not by policy.

The environment orders the model's board at the end of each turn by search with the combat
simulator. It scores each candidate order by its win rate (a draw counts half) against a few
opponent boards: the boards this player has seen, next opponent first, then the most recently seen,
filled up from the probe library's reference boards for this turn when fewer have been seen. Every
candidate uses the same fight seeds (common random numbers). The search is a first-improvement hill
climb over swaps with a fixed budget. It knows nothing about cards.
"""

from __future__ import annotations

import random
from dataclasses import dataclass
from typing import Any

import seaglass

from .probe import ProbeSpec, library_for


@dataclass(frozen=True)
class PositionSpec:
    max_opponents: int = 4  # opponent boards each order is scored against
    sims: int = 2  # fights per (order, opponent board)
    max_evals: int = 16  # candidate orders scored per turn, besides the current one
    library: ProbeSpec | None = ProbeSpec()  # fills the opponent set (None: seen boards only)


def opponent_boards(game: Any, p: int, config: Any, spec: PositionSpec) -> list[tuple[list, int]]:
    """Up to `spec.max_opponents` (board, Tavern Tier) pairs to score orders against."""
    seen = [o for o in game.context(p).opponents if o.alive and len(o.seen_board) > 0]
    seen.sort(key=lambda o: (not o.is_next, -(o.seen_turn if o.seen_turn is not None else -1)))
    out = [(list(o.seen_board), o.tier) for o in seen[: spec.max_opponents]]
    if len(out) < spec.max_opponents and spec.library is not None:
        refs = library_for(spec.library, config).get(game.turn, [])
        out += [(list(b), t) for b, t in refs[: spec.max_opponents - len(out)]]
    return out


def score_order(board: list, opponents: list[tuple[list, int]], tier: int, seed: int, sims: int) -> float:
    """The board's mean score against `opponents`, in [0, 1]."""
    total = 0.0
    for j, (opp, opp_tier) in enumerate(opponents):
        d = seaglass.simulate_batch(board, opp, seed + 7_919 * j, sims, hero_tier_a=tier, hero_tier_b=opp_tier)
        total += d.a_win_rate + 0.5 * d.draw_rate
    return total / len(opponents)


def best_order(
    board: list, opponents: list[tuple[list, int]], tier: int, seed: int, spec: PositionSpec
) -> tuple[list[int], int]:
    """(perm, evals used): the new board is `[board[i] for i in perm]`."""
    n = len(board)
    perm = list(range(n))
    if n < 2 or not opponents:
        return perm, 0
    rng = random.Random(seed)
    best = score_order(board, opponents, tier, seed, spec.sims)
    pairs = [(i, j) for i in range(n) for j in range(i + 1, n)]
    evals = 0
    improved = True
    while improved and evals < spec.max_evals:
        improved = False
        rng.shuffle(pairs)
        for i, j in pairs:
            if evals >= spec.max_evals:
                break
            cand = list(perm)
            cand[i], cand[j] = cand[j], cand[i]
            s = score_order([board[k] for k in cand], opponents, tier, seed, spec.sims)
            evals += 1
            if s > best + 1e-9:
                best, perm, improved = s, cand, True
    return perm, evals


def moves_to(perm: list[int]) -> list[tuple[int, int]]:
    """The Reposition (from, to) moves that turn the board into `[board[i] for i in perm]`. A
    Reposition removes the minion and inserts it at the new position."""
    cur = list(range(len(perm)))
    moves = []
    for t in range(len(perm)):
        f = cur.index(perm[t])
        if f != t:
            moves.append((f, t))
            cur.insert(t, cur.pop(f))
    return moves
