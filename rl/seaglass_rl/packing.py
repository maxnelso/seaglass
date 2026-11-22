"""Packed storage of trajectories: the slot numeric features as CSR rows, the action masks as bits.

A decision's `slot_num_feats` (`NUM_SLOTS x SLOT_NUM_DIM`) is over 97% zeros, and its action mask is
a bool per action. Dense, a decision costs ~21.5 KB as float16 (~40 KB as float32); packed, a few KB.

The numeric features of `T` decisions, flattened to rows of `NUM_SLOTS * SLOT_NUM_DIM` values, are
stored as CSR: `ptr` (int64, `T + 1`), `idx` (uint16, each nonzero's column in its row) and `val`
(float16). The masks are `np.packbits(mask, axis=1)`.
"""

from __future__ import annotations

import numpy as np

from .actions import ACTION_SPACE_SIZE
from .features import NUM_SLOTS, SLOT_NUM_DIM

ROW = NUM_SLOTS * SLOT_NUM_DIM
MASK_BYTES = (ACTION_SPACE_SIZE + 7) // 8
assert ROW <= 65536, "column indices no longer fit in uint16"

Packed = tuple[np.ndarray, np.ndarray, np.ndarray]


def pack_rows(dense: np.ndarray) -> Packed:
    """`(T, NUM_SLOTS, SLOT_NUM_DIM)` (float16) -> `(ptr, idx, val)`."""
    flat = np.ascontiguousarray(dense).reshape(len(dense), ROW)
    rows, cols = np.nonzero(flat)
    ptr = np.zeros(len(flat) + 1, dtype=np.int64)
    np.cumsum(np.bincount(rows, minlength=len(flat)), out=ptr[1:])
    return ptr, cols.astype(np.uint16), flat[rows, cols]


def concat_packed(parts: list[Packed]) -> Packed:
    """Stack the rows of several packed arrays, offsetting the pointers."""
    ptrs = [parts[0][0]]
    offset = int(parts[0][0][-1])
    for ptr, _, _ in parts[1:]:
        ptrs.append(ptr[1:] + offset)
        offset += int(ptr[-1])
    return (
        np.concatenate(ptrs),
        np.concatenate([p[1] for p in parts]),
        np.concatenate([p[2] for p in parts]),
    )


def unpack_rows(ptr: np.ndarray, idx: np.ndarray, val: np.ndarray, rows: np.ndarray) -> np.ndarray:
    """The dense float32 `(len(rows), NUM_SLOTS, SLOT_NUM_DIM)` of the given rows (any order, repeats ok)."""
    rows = np.asarray(rows, dtype=np.int64)
    starts = ptr[rows]
    lens = ptr[rows + 1] - starts
    total = int(lens.sum())
    out = np.zeros((len(rows), ROW), dtype=np.float32)
    if total:
        out_row = np.repeat(np.arange(len(rows)), lens)
        # Position within each row's run: a global arange minus each run's first position.
        within = np.arange(total) - np.repeat(np.cumsum(lens) - lens, lens)
        src = np.repeat(starts, lens) + within
        out[out_row, idx[src]] = val[src]
    return out.reshape(len(rows), NUM_SLOTS, SLOT_NUM_DIM)


def pack_mask(mask: np.ndarray) -> np.ndarray:
    return np.packbits(mask.astype(np.bool_), axis=1)


def unpack_mask(bits: np.ndarray, rows: np.ndarray) -> np.ndarray:
    """The bool `(len(rows), ACTION_SPACE_SIZE)` masks of the given rows."""
    return np.unpackbits(bits[np.asarray(rows, dtype=np.int64)], axis=1, count=ACTION_SPACE_SIZE).astype(np.bool_)
