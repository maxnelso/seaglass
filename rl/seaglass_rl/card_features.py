"""Static per-card features: what each card in the vocabulary *is*, independent of the game state.

`static_card_table()` is a `(VOCAB_SIZE, STATIC_DIM)` float32 array indexed like the card
embedding (`features.CARD_ID_TO_IDX`; row 0 is "no card"). Each row concatenates:
- the card's effect tags from the engine (`seaglass.card_tags()`): which hooks it has (Battlecry,
  Deathrattle, Avenge, Start of Combat, end of turn, auras, ...) and its flags, as 0/1;
- an embedding of the card's rules text (`data/card_text_embeddings.npz`, made by
  `scripts/embed_card_text.py`), all 0 for a card without text (and when the file is missing).

Models look the rows up by card id, so the table never enters a rollout batch.
"""

from __future__ import annotations

import functools
from pathlib import Path

import numpy as np
import seaglass

from .features import CARD_ID_TO_IDX, VOCAB_SIZE

TEXT_EMBEDDINGS = Path(__file__).resolve().parent / "data" / "card_text_embeddings.npz"
# The text embedding width `scripts/embed_card_text.py` writes (its `--dim`).
TEXT_DIM: int = 32

TAG_NAMES: list[str]
_TAGS: dict[int, list[int]]
TAG_NAMES, _TAGS = seaglass.card_tags()
TAG_DIM: int = len(TAG_NAMES)

STATIC_DIM: int = TAG_DIM + TEXT_DIM


def _text_embeddings() -> dict[int, np.ndarray]:
    """Card id -> its text embedding, from `TEXT_EMBEDDINGS` (empty when the file is missing)."""
    if not TEXT_EMBEDDINGS.exists():
        return {}
    data = np.load(TEXT_EMBEDDINGS)
    emb = data["emb"].astype(np.float32)
    if emb.shape[1] != TEXT_DIM:
        raise ValueError(f"{TEXT_EMBEDDINGS}: embeddings are {emb.shape[1]}-wide, expected {TEXT_DIM}")
    return {int(cid): emb[i] for i, cid in enumerate(data["ids"])}


@functools.lru_cache(maxsize=1)
def static_card_table() -> np.ndarray:
    """The `(VOCAB_SIZE, STATIC_DIM)` static feature table."""
    table = np.zeros((VOCAB_SIZE, STATIC_DIM), dtype=np.float32)
    text = _text_embeddings()
    for cid, idx in CARD_ID_TO_IDX.items():
        tags = _TAGS.get(cid)
        if tags is not None:
            table[idx, :TAG_DIM] = tags
        emb = text.get(cid)
        if emb is not None:
            table[idx, TAG_DIM:] = emb
    return table
