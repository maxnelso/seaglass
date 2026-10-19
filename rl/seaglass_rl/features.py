"""Observation encoding with dynamic card-ID vocabulary (`nn.Embedding` + numeric slot features)."""

from __future__ import annotations

import math
import numpy as np
import seaglass

from .actions import (
    MAX_BOARD,
    MAX_DISCOVER,
    MAX_HAND,
    MAX_HERO_OPTIONS,
    MAX_SHOP,
    MAX_TRINKET_OPTIONS,
)

# Index 0 is reserved for empty slots / unknown cards.
_ALL_IDS = seaglass.all_card_ids()
CARD_ID_TO_IDX: dict[int, int] = {cid: i + 1 for i, cid in enumerate(_ALL_IDS)}
# The heroes (`engine/docs/heroes.md` §2.1), appended after the cards: the hero and hero option
# slots embed them like cards (dbfIds never collide).
HERO_IDS: list[int] = [h[0] for h in seaglass.heroes_table()]
for _hid in HERO_IDS:
    CARD_ID_TO_IDX.setdefault(_hid, len(CARD_ID_TO_IDX) + 1)
VOCAB_SIZE: int = len(CARD_ID_TO_IDX) + 1

# The cards waiting for room in the hand (`engine/docs/tavern.md` §7.4), appended last.
MAX_WAITING: int = 3
# The owned trinkets, then the offered ones (`engine/docs/trinkets.md` §8.2), appended after them.
MAX_TRINKETS: int = 2
# The slots of the player's own Tavern (the first ones, in this order).
TAVERN_SLOTS: int = (
    MAX_BOARD + MAX_HAND + MAX_SHOP + MAX_DISCOVER + MAX_WAITING + MAX_TRINKETS + MAX_TRINKET_OPTIONS
)  # 7 + 10 + 7 + 4 + 3 + 2 + 4 = 37
# Appended after them: the hero and the offered heroes (`engine/docs/heroes.md` §6), the Duos
# teammate's board and hand (empty in Solo), and the next opponents' boards as they entered their
# last combat (Solo: the next opponent; Duos: the opposing player in this seat, then their teammate).
HERO_SLOTS: int = 1 + MAX_HERO_OPTIONS
TEAMMATE_SLOTS: int = MAX_BOARD + MAX_HAND
OPPONENT_BOARD_SLOTS: int = 2 * MAX_BOARD
HERO_OFFSET: int = TAVERN_SLOTS
TEAMMATE_OFFSET: int = HERO_OFFSET + HERO_SLOTS
OPPONENT_BOARD_OFFSET: int = TEAMMATE_OFFSET + TEAMMATE_SLOTS
NUM_SLOTS: int = OPPONENT_BOARD_OFFSET + OPPONENT_BOARD_SLOTS  # 37 + 5 + 17 + 14 = 73
# The Dark Gifts (`engine/docs/dark_gifts.md` §8.2), in their append-only vocabulary order: each card
# slot's last features count the copies of each gift on the card.
DARK_GIFT_IDS: list[int] = seaglass.dark_gifts_vocabulary()
DARK_GIFT_TO_FEATURE: dict[int, int] = {gid: i for i, gid in enumerate(DARK_GIFT_IDS)}
# The Dark Discovery button's uses left (/ 3) and whether it is usable now, appended last.
TAVERN_PLAYER_DIM: int = 33
# Appended after them (`seaglass.LobbyContext`): the lobby, each opponent, and the Duos team.
LOBBY_DIM: int = 8
MAX_OPPONENTS: int = 7
OPPONENT_DIM: int = 7
DUOS_DIM: int = 11
LOBBY_OFFSET: int = TAVERN_PLAYER_DIM
OPPONENTS_OFFSET: int = LOBBY_OFFSET + LOBBY_DIM
DUOS_OFFSET: int = OPPONENTS_OFFSET + MAX_OPPONENTS * OPPONENT_DIM
PLAYER_DIM: int = DUOS_OFFSET + DUOS_DIM  # 33 + 8 + 49 + 11 = 101
BASE_SLOT_NUM_DIM: int = 14
# The minion type a trinket names (`engine/docs/trinkets.md` §3.4): a one-hot of the 10 lobby types,
# appended after the Dark Gift features, set only in the trinket slots (owned and offered).
NUM_TRINKET_TRIBES: int = 10
TRINKET_TRIBE_OFFSET: int = BASE_SLOT_NUM_DIM + len(DARK_GIFT_IDS)
# The stats a card gains at the end of each turn from an effect given to it (`Gruul Rules`,
# `engine/docs/darkmoon_prizes.md` §4.2), log-scaled, appended after the trinket types.
END_OF_TURN_STATS_OFFSET: int = TRINKET_TRIBE_OFFSET + NUM_TRINKET_TRIBES
SLOT_NUM_DIM: int = END_OF_TURN_STATS_OFFSET + 1


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
    out_num[END_OF_TURN_STATS_OFFSET] = math.log1p(max(0, u.end_of_turn_stats)) / 5.0
    return CARD_ID_TO_IDX.get(u.card_id, 0)


def _encode_trinket_tribe(tribes: list[int], i: int, out_num: np.ndarray) -> None:
    """Set the one-hot of the minion type the `i`th trinket names, if it names one."""
    if i < len(tribes) and 1 <= tribes[i] <= NUM_TRINKET_TRIBES:
        out_num[TRINKET_TRIBE_OFFSET + tribes[i] - 1] = 1.0


def encode_observation(
    obs: seaglass.TavernObservation | seaglass.DuosObservation,
    context: seaglass.LobbyContext | None = None,
) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Encode a `TavernObservation` (or a `DuosObservation`) into `(player_vec, slot_card_ids,
    slot_num_feats)`.

    `context` is the player's view of their lobby (`seaglass.LobbyGame.context`). Without one, the
    lobby features are 0. A `DuosObservation` also fills the teammate's features and slots.

    Returns:
      - `player_vec`: float32 array of shape `(PLAYER_DIM,)`
      - `slot_card_ids`: int64 array of shape `(NUM_SLOTS,)` (indices in `0..VOCAB_SIZE`)
      - `slot_num_feats`: float32 array of shape `(NUM_SLOTS, SLOT_NUM_DIM)`
    """
    option_costs = [c / 10.0 for c in obs.trinket_option_costs[:MAX_TRINKET_OPTIONS]]
    option_costs += [0.0] * (MAX_TRINKET_OPTIONS - len(option_costs))
    player_vec = np.zeros(PLAYER_DIM, dtype=np.float32)
    player_vec[:TAVERN_PLAYER_DIM] = [
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
    ]

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

    # The hero, then the offered heroes: a present flag and the hero's id.
    for i, hid in enumerate([obs.hero_id, *obs.hero_options[:MAX_HERO_OPTIONS]]):
        if hid:
            slot_card_ids[HERO_OFFSET + i] = CARD_ID_TO_IDX.get(hid, 0)
            slot_num_feats[HERO_OFFSET + i, 0] = 1.0

    teammate_board = getattr(obs, "teammate_board", None)
    if teammate_board is not None:
        _encode_duos(obs, player_vec, slot_card_ids, slot_num_feats)
    if context is not None:
        _encode_lobby(context, player_vec, slot_card_ids, slot_num_feats)

    return player_vec, slot_card_ids, slot_num_feats


def _encode_duos(
    obs: seaglass.DuosObservation,
    player_vec: np.ndarray,
    slot_card_ids: np.ndarray,
    slot_num_feats: np.ndarray,
) -> None:
    """The Duos team (`engine/docs/duos_tavern.md`): the teammate's Tavern as the player sees it,
    the Passes and the first fighter."""
    player_vec[DUOS_OFFSET : DUOS_OFFSET + DUOS_DIM] = [
        1.0,  # a Duos game
        obs.teammate_tavern_tier / 7.0,
        obs.teammate_gold / 10.0,
        1.0 if obs.teammate_ended else 0.0,
        1.0 if obs.teammate_hero_id else 0.0,
        1.0 if obs.fights_first else 0.0,
        obs.free_passes / 3.0,
        len(obs.incoming) / 3.0,
        len(obs.teammate_incoming) / 3.0,
        obs.passes_this_turn / 3.0,
        obs.passes_this_game / 10.0,
    ]
    for i, u in enumerate(obs.teammate_board[:MAX_BOARD]):
        slot = TEAMMATE_OFFSET + i
        slot_card_ids[slot] = _encode_unit_into(u, slot_num_feats[slot])
    for i, u in enumerate(obs.teammate_hand[:MAX_HAND]):
        slot = TEAMMATE_OFFSET + MAX_BOARD + i
        slot_card_ids[slot] = _encode_unit_into(u, slot_num_feats[slot])


def _encode_lobby(
    context: seaglass.LobbyContext,
    player_vec: np.ndarray,
    slot_card_ids: np.ndarray,
    slot_num_feats: np.ndarray,
) -> None:
    """The lobby (`seaglass.LobbyContext`): who is left, each opponent as the leaderboard shows them,
    and the next opponents' last boards."""
    nxt = [o for o in context.opponents if o.is_next]
    first = nxt[0] if nxt else None
    player_vec[LOBBY_OFFSET : LOBBY_OFFSET + LOBBY_DIM] = [
        1.0,  # a lobby game
        context.units / 8.0,
        context.alive_units / 8.0,
        (context.health_rank - 1) / 7.0,
        1.0 if context.next_is_ghost else 0.0,
        first.health / 30.0 if first else 0.0,
        first.armor / 20.0 if first else 0.0,
        first.tier / 7.0 if first else 0.0,
    ]
    for i, o in enumerate(context.opponents[:MAX_OPPONENTS]):
        at = OPPONENTS_OFFSET + i * OPPONENT_DIM
        player_vec[at : at + OPPONENT_DIM] = [
            1.0,
            1.0 if o.alive else 0.0,
            o.health / 30.0,
            o.armor / 20.0,
            o.tier / 7.0,
            1.0 if o.is_next else 0.0,
            1.0 if o.is_last else 0.0,
        ]
    for k, o in enumerate(nxt[:2]):
        for i, u in enumerate(o.last_board[:MAX_BOARD]):
            slot = OPPONENT_BOARD_OFFSET + k * MAX_BOARD + i
            slot_card_ids[slot] = _encode_unit_into(u, slot_num_feats[slot])
