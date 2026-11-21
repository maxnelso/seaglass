"""The per-turn board probe.

Placement arrives once per game and is noisy. Board strength by turn is a dense signal. The probe
fights each end-of-turn board against a frozen library of reference boards from the same turn, and
reports the win rate (a draw counts half).

The library is built from lobbies of the pinned rule bots on fixed seeds, so it is reproducible at a
given commit. It is rebuilt deterministically instead of stored, because `seaglass.Unit` can't be
serialized. Each worker process builds it once.

The fights are board-only (`seaglass.simulate_batch`, with both Tavern Tiers). Hero Powers, trinkets
and other start-of-combat effects outside the board are not included, so the probe measures the
warband, not the whole player. Late turns have only survivors in both the library and the probed
boards.
"""

from __future__ import annotations

import random
from dataclasses import dataclass
from typing import Any

import seaglass

from . import play
from .play import GameSpec, LobbyConfig, PlayOptions


@dataclass(frozen=True)
class ProbeSpec:
    """How the reference library is built and used."""

    lobbies: int = 32  # reference lobbies played to build the library
    seed: int = 31_000_000  # their seeds are seed + i
    bots: tuple[str, ...] = ("9kgamerbot", "9kgamerbot-tempo", "heuristic")
    per_turn: int = 32  # reference boards kept per turn
    min_boards: int = 16  # turns with fewer reference boards are not probed
    sims: int = 2  # fights per (board, reference) pair


# turn -> [(board units, Tavern Tier)]
Library = dict[int, list[tuple[list[Any], int]]]


def build_library(spec: ProbeSpec, config: LobbyConfig) -> Library:
    """Play `spec.lobbies` bot lobbies and keep up to `spec.per_turn` end-of-turn boards per turn."""
    if config.duos:
        raise ValueError("the probe is Solo-only")
    specs = [
        GameSpec(
            seed=spec.seed + i,
            controllers=[spec.bots[(i + u) % len(spec.bots)] for u in range(config.units)],
        )
        for i in range(spec.lobbies)
    ]
    boards: dict[int, list[tuple[list[Any], int]]] = {}

    def collect(g: int, p: int, game: seaglass.LobbyGame, obs: Any) -> None:
        boards.setdefault(game.turn, []).append((list(obs.board), obs.tavern_tier))

    play.play_games(config, specs, {}, PlayOptions(), record=(), on_turn_end=collect)
    library: Library = {}
    for turn in sorted(boards):
        refs = boards[turn]
        if len(refs) < spec.min_boards:
            continue
        if len(refs) > spec.per_turn:
            refs = random.Random(spec.seed + turn).sample(refs, spec.per_turn)
        library[turn] = refs
    return library


_CACHE: dict[tuple[ProbeSpec, str], Library] = {}


def library_for(spec: ProbeSpec, config: LobbyConfig) -> Library:
    """The library for `spec` and `config`, built once per process."""
    key = (spec, repr(config))
    if key not in _CACHE:
        _CACHE[key] = build_library(spec, config)
    return _CACHE[key]


def score_board(library: Library, board: list, tier: int, turn: int, seed: int, sims: int) -> float | None:
    """The board's mean score against the turn's reference boards, in [0, 1] (a win is 1, a draw
    0.5). None when the library has no boards for the turn."""
    refs = library.get(turn)
    if not refs:
        return None
    total = 0.0
    for i, (ref, ref_tier) in enumerate(refs):
        d = seaglass.simulate_batch(board, ref, seed + i, sims, hero_tier_a=tier, hero_tier_b=ref_tier)
        total += d.a_win_rate + 0.5 * d.draw_rate
    return total / len(refs)


def library_summary(library: Library) -> dict[int, int]:
    """The number of reference boards kept per turn."""
    return {turn: len(refs) for turn, refs in sorted(library.items())}
