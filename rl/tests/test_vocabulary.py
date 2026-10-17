"""The RL card vocabulary is append-only (`engine/docs/trinkets.md` §8.2)."""

from __future__ import annotations

from pathlib import Path

import seaglass


def test_the_card_vocabulary_only_grows_at_the_end() -> None:
    lines = (Path(__file__).parent / "data" / "card_ids_v1.txt").read_text().splitlines()
    first = [int(line) for line in lines if line and not line.startswith("#")]
    ids = seaglass.all_card_ids()
    assert ids[: len(first)] == first
    assert len(set(ids)) == len(ids)
