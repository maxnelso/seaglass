"""What a policy actually does, turn by turn: how much it rolls, buys, sells, levels and leaves unspent,
from recorded trajectories (`play.Trajectory.arrays`). It shows what a policy has learned (e.g. whether
it stopped rolling early, or levels on a sensible curve) where its placement alone can't.
"""

from __future__ import annotations

from collections import defaultdict
from typing import Any

import numpy as np

from .actions import ACTIONS

# Turn buckets the per-turn rates are reported for (inclusive bounds).
BUCKETS: list[tuple[int, int]] = [(1, 3), (4, 6), (7, 9), (10, 99)]
# The turns the Tavern Tier is reported at.
TIER_TURNS: list[int] = [3, 5, 7, 9, 11]
# The action kinds counted per turn.
KINDS: list[str] = ["refresh", "buy", "sell", "upgrade_tavern", "reposition", "toggle_freeze", "hero_power", "pass"]


def _bucket(turn: int) -> str | None:
    for lo, hi in BUCKETS:
        if lo <= turn <= hi:
            return f"T{lo}-{hi}" if hi < 99 else f"T{lo}+"
    return None


def behavior_stats(trajectories: list[dict[str, np.ndarray]]) -> dict[str, Any]:
    """Per-turn action rates by turn bucket, the Tavern Tier at given turns, and the Gold left when
    ending a turn, over every player-turn in `trajectories` (the hero choice is skipped).

    The turn and Tier come from the observation's player vector (`features.encode_observation`:
    `turn / 20`, `tier / 6`, `gold / 10` are its features 0, 3 and 4).
    """
    counts: dict[str, dict[str, float]] = defaultdict(lambda: defaultdict(float))
    turns_in: dict[str, int] = defaultdict(int)
    tiers: dict[int, list[float]] = defaultdict(list)
    unspent: list[float] = []
    actions_per_turn: list[int] = []
    for tr in trajectories:
        pv, acts = tr["player_vec"], tr["actions"]
        turns = np.rint(pv[:, 0] * 20).astype(int)
        start = 0
        while start < len(acts):
            turn = turns[start]
            end = start
            while end < len(acts) and turns[end] == turn:
                end += 1
            if turn >= 1:
                bucket = _bucket(int(turn))
                kinds = [ACTIONS[a].kind for a in acts[start:end]]
                if bucket:
                    turns_in[bucket] += 1
                    for k in kinds:
                        if k in KINDS:
                            counts[bucket][k] += 1
                if int(turn) in TIER_TURNS:
                    tiers[int(turn)].append(float(pv[start, 3] * 6))
                if kinds and kinds[-1] == "end_turn":
                    unspent.append(float(pv[end - 1, 4] * 10))
                actions_per_turn.append(end - start)
            start = end
    per_turn = {
        b: {k: round(counts[b][k] / turns_in[b], 3) for k in KINDS} for b in turns_in if turns_in[b]
    }
    return {
        "per_turn": per_turn,
        "tier_at": {t: round(float(np.mean(v)), 2) for t, v in sorted(tiers.items()) if v},
        "unspent_gold": round(float(np.mean(unspent)), 2) if unspent else None,
        "actions_per_turn": round(float(np.mean(actions_per_turn)), 2) if actions_per_turn else None,
    }


def format_behavior(stats: dict[str, Any]) -> str:
    """A compact multi-line summary of `behavior_stats`."""
    lines = []
    per_turn = stats["per_turn"]
    for kind, label in [("refresh", "rolls"), ("buy", "buys"), ("sell", "sells"), ("upgrade_tavern", "levels"),
                        ("reposition", "moves"), ("hero_power", "hero pw")]:
        cells = " ".join(f"{b} {per_turn[b][kind]:.2f}" for b in per_turn)
        lines.append(f"    {label:8s}/turn: {cells}")
    tiers = " ".join(f"T{t}:{v:.1f}" for t, v in stats["tier_at"].items())
    lines.append(
        f"    tier at {tiers} | gold left at end turn {stats['unspent_gold']} | actions/turn {stats['actions_per_turn']}"
    )
    return "\n".join(lines)
