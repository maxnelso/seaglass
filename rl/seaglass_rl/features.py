"""Observation encoding with dynamic card-ID vocabulary (`nn.Embedding` + numeric slot features)."""

from __future__ import annotations

import math
import numpy as np
import seaglass

from .actions import MAX_BOARD, MAX_DISCOVER, MAX_HAND, MAX_SHOP, MAX_TRINKET_OPTIONS

# Index 0 is reserved for empty slots / unknown cards.
_ALL_IDS = seaglass.all_card_ids()
CARD_ID_TO_IDX: dict[int, int] = {cid: i + 1 for i, cid in enumerate(_ALL_IDS)}
VOCAB_SIZE: int = len(_ALL_IDS) + 1

# The cards waiting for room in the hand (`engine/docs/tavern.md` §7.4), appended last.
MAX_WAITING: int = 3
# The owned trinkets, then the offered ones (`engine/docs/trinkets.md` §8.2), appended after them.
MAX_TRINKETS: int = 2
NUM_SLOTS: int = (
    MAX_BOARD + MAX_HAND + MAX_SHOP + MAX_DISCOVER + MAX_WAITING + MAX_TRINKETS + MAX_TRINKET_OPTIONS
)  # 7 + 10 + 7 + 4 + 3 + 2 + 4 = 37
# The Dark Gifts (`engine/docs/dark_gifts.md` §8.2), in their append-only vocabulary order: each card
# slot's last features count the copies of each gift on the card.
DARK_GIFT_IDS: list[int] = seaglass.dark_gifts_vocabulary()
DARK_GIFT_TO_FEATURE: dict[int, int] = {gid: i for i, gid in enumerate(DARK_GIFT_IDS)}
# The Dark Discovery button's uses left (/ 3) and whether it is usable now, appended last.
PLAYER_DIM: int = 33
BASE_SLOT_NUM_DIM: int = 14
# The minion type a trinket names (`engine/docs/trinkets.md` §3.4): a one-hot of the 10 lobby types,
# appended after the Dark Gift features, set only in the trinket slots (owned and offered).
NUM_TRINKET_TRIBES: int = 10
TRINKET_TRIBE_OFFSET: int = BASE_SLOT_NUM_DIM + len(DARK_GIFT_IDS)
SLOT_NUM_DIM: int = TRINKET_TRIBE_OFFSET + NUM_TRINKET_TRIBES


def _encode_unit_into(u: seaglass.Unit, out_num: np.ndarray) -> int:
    """Fill `out_num` (length `SLOT_NUM_DIM`) for unit `u` and return its vocab index."""
    out_num[0] = 1.0  # present flag
    out_num[1] = math.log1p(max(0, u.attack)) / 5.0
    out_num[2] = math.log1p(max(0, u.health)) / 5.0
    out_num[3] = u.tavern_tier / 7.0
    out_num[4] = 1.0 if u.is_spell else 0.0
    out_num[5] = 1.0 if u.is_golden else 0.0
    out_num[6] = 1.0 if u.taunt else 0.0
    out_num[7] = 1.0 if u.divine_shield else 0.0
    out_num[8] = 1.0 if u.windfury else 0.0
    out_num[9] = 1.0 if u.reborn else 0.0
    out_num[10] = 1.0 if u.venomous else 0.0
    out_num[11] = 1.0 if u.magnetic else 0.0
    out_num[12] = 1.0 if u.frozen else 0.0  # a shop card frozen on its own
    out_num[13] = 1.0 if u.marked else 0.0  # marked by the Hero Power (a Silas ticket, Vol'jin's pick)
    for gid in u.dark_gifts:  # the card's Dark Gifts, one feature per gift
        feature = DARK_GIFT_TO_FEATURE.get(gid)
        if feature is not None:
            out_num[BASE_SLOT_NUM_DIM + feature] += 1.0
    return CARD_ID_TO_IDX.get(u.card_id, 0)


def _encode_trinket_tribe(tribes: list[int], i: int, out_num: np.ndarray) -> None:
    """Set the one-hot of the minion type the `i`th trinket names, if it names one."""
    if i < len(tribes) and 1 <= tribes[i] <= NUM_TRINKET_TRIBES:
        out_num[TRINKET_TRIBE_OFFSET + tribes[i] - 1] = 1.0


def encode_observation(obs: seaglass.TavernObservation) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Encode a `TavernObservation` into `(player_vec, slot_card_ids, slot_num_feats)`.

    Returns:
      - `player_vec`: float32 array of shape `(PLAYER_DIM,)`
      - `slot_card_ids`: int64 array of shape `(NUM_SLOTS,)` (indices in `0..VOCAB_SIZE`)
      - `slot_num_feats`: float32 array of shape `(NUM_SLOTS, SLOT_NUM_DIM)`
    """
    option_costs = [c / 10.0 for c in obs.trinket_option_costs[:MAX_TRINKET_OPTIONS]]
    option_costs += [0.0] * (MAX_TRINKET_OPTIONS - len(option_costs))
    player_vec = np.array(
        [
            obs.turn / 20.0,
            obs.health / 30.0,
            obs.armor / 10.0,
            obs.tavern_tier / 6.0,
            obs.gold / 10.0,
            obs.max_gold / 10.0,
            obs.upgrade_cost / 10.0,
            obs.free_refreshes / 5.0,
            1.0 if obs.discover_pending is not None else 0.0,
            # The hero and its Hero Power (`engine/docs/heroes.md` §8); all 0 without a hero.
            1.0 if obs.hero_id else 0.0,
            1.0 if obs.hero_power_usable else 0.0,
            obs.hero_power_cost / 10.0,
            float(obs.hero_power_uses_left),
            obs.hero_power_armed / 3.0,
            1.0 if obs.hero_options else 0.0,
            # Freezing (`engine/docs/tavern.md` §3.1): the shop as a whole.
            1.0 if obs.is_frozen else 0.0,
            # The next minion's discount (`engine/docs/tavern.md` §6): Aranna's free minion.
            obs.next_minion_discount / 3.0,
            # The cards waiting for room in the hand (Spellcraft spells, `engine/docs/tavern.md`
            # §7.4); the first `MAX_WAITING` are also the last slots.
            len(obs.waiting_for_hand) / MAX_WAITING,
            # The prices a Hero Power can change (`engine/docs/tavern.md` §3.3): Sindragosa,
            # Millhouse.
            obs.minion_cost / 3.0,
            obs.refresh_cost / 2.0,
            # The second Hero Power (`engine/docs/heroes.md` §3, §8); all 0 without one.
            1.0 if obs.second_hero_power_id else 0.0,
            1.0 if obs.second_hero_power_usable else 0.0,
            obs.second_hero_power_cost / 10.0,
            float(obs.second_hero_power_uses_left),
            obs.second_hero_power_armed / 3.0,
            # Trinkets (`engine/docs/trinkets.md` §8.2): a pending offer, the trinkets owned, and
            # the offered prices (0 for no option).
            1.0 if obs.trinket_options else 0.0,
            len(obs.trinkets) / MAX_TRINKETS,
            *option_costs,
            # The Dark Discovery button (`engine/docs/dark_gifts.md` §8.2); 0 without Dark Gifts.
            obs.dark_discoveries_left / 3.0,
            1.0 if obs.dark_discovery_usable else 0.0,
        ],
        dtype=np.float32,
    )

    slot_card_ids = np.zeros(NUM_SLOTS, dtype=np.int64)
    slot_num_feats = np.zeros((NUM_SLOTS, SLOT_NUM_DIM), dtype=np.float32)

    offset = 0
    for i, u in enumerate(obs.board[:MAX_BOARD]):
        slot_card_ids[offset + i] = _encode_unit_into(u, slot_num_feats[offset + i])

    offset += MAX_BOARD
    for i, u in enumerate(obs.hand[:MAX_HAND]):
        slot_card_ids[offset + i] = _encode_unit_into(u, slot_num_feats[offset + i])

    offset += MAX_HAND
    for i, u in enumerate(obs.shop[:MAX_SHOP]):
        slot_card_ids[offset + i] = _encode_unit_into(u, slot_num_feats[offset + i])

    offset += MAX_SHOP
    if obs.discover_pending is not None:
        for i, u in enumerate(obs.discover_pending[:MAX_DISCOVER]):
            slot_card_ids[offset + i] = _encode_unit_into(u, slot_num_feats[offset + i])

    offset += MAX_DISCOVER
    for i, u in enumerate(obs.waiting_for_hand[:MAX_WAITING]):
        slot_card_ids[offset + i] = _encode_unit_into(u, slot_num_feats[offset + i])

    # Trinkets: a present flag, the card id (their ids are in the vocabulary) and the minion type
    # the trinket names (`trinket_tribes`: 1 + its index in the lobby types, 0 for none).
    offset += MAX_WAITING
    for i, tid in enumerate(obs.trinkets[:MAX_TRINKETS]):
        slot_card_ids[offset + i] = CARD_ID_TO_IDX.get(tid, 0)
        slot_num_feats[offset + i, 0] = 1.0
        _encode_trinket_tribe(obs.trinket_tribes, i, slot_num_feats[offset + i])
    offset += MAX_TRINKETS
    for i, tid in enumerate(obs.trinket_options[:MAX_TRINKET_OPTIONS]):
        slot_card_ids[offset + i] = CARD_ID_TO_IDX.get(tid, 0)
        slot_num_feats[offset + i, 0] = 1.0
        _encode_trinket_tribe(obs.trinket_option_tribes, i, slot_num_feats[offset + i])

    return player_vec, slot_card_ids, slot_num_feats
