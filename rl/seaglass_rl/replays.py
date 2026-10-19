"""Recorded matches as data: load replays (`engine/docs/replays.md`) and turn their decisions into
arrays shaped like the trainer's batches (`player_vec`, `slot_card_ids`, `slot_num_feats`,
`action_mask`, `actions`)."""

from __future__ import annotations

import math
import os
import warnings
from collections.abc import Iterable, Iterator
from dataclasses import dataclass
from typing import Any, Union

import numpy as np
import seaglass

from .actions import ACTION_SPACE_SIZE, ACTION_TO_INDEX, action_mask
from .features import NUM_SLOTS, PLAYER_DIM, SLOT_NUM_DIM, encode_observation

# A replay file, or one already loaded.
Source = Union[str, "os.PathLike[str]", seaglass.Timeline]


def load(path: str | os.PathLike[str], strict: bool = True) -> seaglass.Timeline:
    """Load and re-simulate a replay file.

    With `strict`, raise `ValueError` unless this engine reproduces the replay exactly. Replays
    only need to work on the commit that recorded them; one from another build also warns.
    """
    timeline = seaglass.Timeline.load(path)
    check(timeline, os.fspath(path), strict=strict)
    return timeline


def check(timeline: seaglass.Timeline, label: str = "replay", strict: bool = True) -> None:
    """Warn if `timeline` was recorded by another engine build; with `strict`, raise
    `ValueError` if it did not re-simulate faithfully."""
    if timeline.engine_mismatch is not None:
        warnings.warn(
            f"{label}: recorded by engine {timeline.engine_mismatch}, this is {seaglass.ENGINE_VERSION}",
            stacklevel=3,
        )
    if strict and not timeline.is_faithful:
        problems = [f"frame {frame}: {message}" for _, frame, message in timeline.divergences]
        if timeline.error is not None:
            record, message = timeline.error
            problems.append(f"record {record}: {message}")
        more = f" (and {len(problems) - 3} more)" if len(problems) > 3 else ""
        raise ValueError(f"{label} does not replay faithfully: {'; '.join(problems[:3])}{more}")


def match_outcome(timeline: seaglass.Timeline) -> list[float]:
    """Per player: 1.0 for the winner, -1.0 for the others, all 0.0 for a draw or a match that
    was not finished."""
    last = timeline[-1]
    if last.kind != "end" or last.winner is None:
        return [0.0] * timeline.num_players
    return [1.0 if p == last.winner else -1.0 for p in range(timeline.num_players)]


@dataclass(frozen=True)
class Decision:
    """One action of a recorded match."""

    timeline: seaglass.Timeline
    source: int  # The replay's index in the sources passed to `iter_decisions`.
    frame: int  # The frame the action produced; it was taken in frame `frame - 1`.
    player: int
    action: seaglass.TavernAction
    note: dict[str, Any] | None

    @property
    def before(self) -> seaglass.FramePlayer:
        """The acting player just before the action (`observe()`, `legal_actions()`, ...)."""
        return self.timeline.player_at(self.frame - 1, self.player)

    @property
    def forced(self) -> bool:
        """Whether the action was made for the player (e.g. at the per-turn step cap)."""
        return bool(self.note and self.note.get("forced"))


def _player_index(timeline: seaglass.Timeline, player: int | str | None) -> int | None:
    if player is None or isinstance(player, int):
        return player
    names = [p["name"] for p in timeline.header["players"]]
    return names.index(player) if player in names else -1


def iter_decisions(
    sources: Source | Iterable[Source],
    player: int | str | None = None,
    strict: bool = True,
) -> Iterator[Decision]:
    """Every action of `sources` (replay paths or `Timeline`s), in order.

    `player` keeps one player's actions: an index, or a name from the replay header (replays
    without a player of that name are skipped). `strict` is passed to `load`.
    """
    if isinstance(sources, (str, os.PathLike, seaglass.Timeline)):
        sources = [sources]
    for i, src in enumerate(sources):
        if isinstance(src, seaglass.Timeline):
            timeline = src
            check(timeline, f"timeline {i}", strict=strict)
        else:
            timeline = load(src, strict=strict)
        wanted = _player_index(timeline, player)
        for frame, p, action, note in timeline.decisions():
            if wanted is None or p == wanted:
                yield Decision(timeline, i, frame, p, action, note)


def to_dataset(
    sources: Source | Iterable[Source],
    player: int | str | None = None,
    prune_dominated: bool = True,
    strict: bool = True,
) -> dict[str, np.ndarray]:
    """The decisions of `sources` as arrays, one row per action (see `iter_decisions`).

    - `player_vec`, `slot_card_ids`, `slot_num_feats`: `encode_observation` of the acting player
      just before the action.
    - `action_mask`: `action_mask` there, with the env's settings (`prune_dominated`, no
      reposition or freeze), plus the taken action so it always has support.
    - `actions`: the action's index in the action space.
    - `value`, `logp`: from the action's note (NaN when absent); `forced`: made for the player.
    - `outcome`: `match_outcome` for the acting player (1 won, -1 lost, 0 draw or unfinished).
    - `player`, `turn`, `source` (index into `sources`), `frame`: where the row comes from.

    Actions outside the action space are skipped with a warning.
    """
    cols: dict[str, list] = {
        k: []
        for k in (
            "player_vec",
            "slot_card_ids",
            "slot_num_feats",
            "action_mask",
            "actions",
            "value",
            "logp",
            "forced",
            "outcome",
            "player",
            "turn",
            "source",
            "frame",
        )
    }
    outcomes: dict[int, list[float]] = {}
    skipped = 0
    for d in iter_decisions(sources, player=player, strict=strict):
        arg0, arg1 = d.action.args
        index = ACTION_TO_INDEX.get((d.action.kind, arg0, arg1))
        if index is None:
            skipped += 1
            continue
        before = d.before
        obs = before.observe()
        player_vec, slot_card_ids, slot_num_feats = encode_observation(obs)
        mask = action_mask(before, prune_dominated=prune_dominated, obs=obs)
        mask[index] = True
        note = d.note or {}
        if d.source not in outcomes:
            outcomes[d.source] = match_outcome(d.timeline)

        cols["player_vec"].append(player_vec)
        cols["slot_card_ids"].append(slot_card_ids)
        cols["slot_num_feats"].append(slot_num_feats)
        cols["action_mask"].append(mask)
        cols["actions"].append(index)
        cols["value"].append(note.get("value", math.nan))
        cols["logp"].append(note.get("logp", math.nan))
        cols["forced"].append(bool(note.get("forced", False)))
        cols["outcome"].append(outcomes[d.source][d.player])
        cols["player"].append(d.player)
        cols["turn"].append(obs.turn)
        cols["source"].append(d.source)
        cols["frame"].append(d.frame)

    if skipped:
        warnings.warn(f"skipped {skipped} actions outside the action space", stacklevel=2)

    shapes = {
        "player_vec": ((PLAYER_DIM,), np.float32),
        "slot_card_ids": ((NUM_SLOTS,), np.int64),
        "slot_num_feats": ((NUM_SLOTS, SLOT_NUM_DIM), np.float32),
        "action_mask": ((ACTION_SPACE_SIZE,), np.bool_),
        "actions": ((), np.int64),
        "value": ((), np.float32),
        "logp": ((), np.float32),
        "forced": ((), np.bool_),
        "outcome": ((), np.float32),
        "player": ((), np.int64),
        "turn": ((), np.int64),
        "source": ((), np.int64),
        "frame": ((), np.int64),
    }
    return {k: np.asarray(cols[k], dtype=dtype).reshape((-1, *shape)) for k, (shape, dtype) in shapes.items()}
