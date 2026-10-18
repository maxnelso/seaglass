"""Darkmoon Prizes in the RL encoding (`engine/docs/darkmoon_prizes.md` §8.2)."""

from __future__ import annotations

import math

import numpy as np
import seaglass

from seaglass_rl import SLOT_NUM_DIM
from seaglass_rl.features import END_OF_TURN_STATS_OFFSET, _encode_unit_into


def test_the_prizes_are_in_the_card_vocabulary() -> None:
    ids = set(seaglass.all_card_ids())
    # Gacha Gift, Big Winner!, All-Seeing Elder and Corrupted Tome's Triple Prize.
    assert {65236, 66460, 122104, 130856} <= ids


def test_gruul_rules_end_of_turn_stats_are_a_slot_feature() -> None:
    assert END_OF_TURN_STATS_OFFSET == SLOT_NUM_DIM - 1
    unit = seaglass.Unit("Joyous", 2, 3)
    out = np.zeros(SLOT_NUM_DIM, dtype=np.float32)
    _encode_unit_into(unit, out)
    assert out[END_OF_TURN_STATS_OFFSET] == 0.0
    unit.end_of_turn_stats = 4
    assert unit.end_of_turn_stats == 4
    _encode_unit_into(unit, out)
    assert math.isclose(out[END_OF_TURN_STATS_OFFSET], math.log1p(4) / 5.0, rel_tol=1e-6)
