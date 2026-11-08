"""`9kgamerbot`: a rule-based bot that levels, rolls for a build-around card, then commits to it.

The plan, and the lynchpin list (`DIRECTIONS`): `docs/9kgamerbot.md`. In short:

1. **Level** up to Tavern Tier 4: every turn it can afford to, or (`tempo_levels`) only when it can
   still buy a minion afterwards. It spends the rest of its Gold on the most stats: the Dark
   Discovery button when the shop has nothing it is looking for, else the best minion (no Tavern
   spells until it commits). Once the board is full, it sells its weakest minion for a clearly
   better one.
2. **Roll for a lynchpin.** At Tier 4 with a full board, it rolls until a *lynchpin* of a direction
   whose minion type is in the lobby shows up, keeping enough Gold to buy it, and freezes one it
   can't afford. Still uncommitted on turn `level5_turn`, it levels to Tier 5 and rolls there.
3. **Commit.** Once it owns a lynchpin (at Tier 4+), it prefers that direction's cards to raw
   stats, sells minions outside it for ones inside it, keeps pairs for triples, rolls for direction
   cards, and levels to Tier 5 and then 6 when it feels strong (`_strong`), or anyway by the
   `late_levels` turns.

It never Passes cards (Duos) and never repositions: two things a later version could add.
"""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass, field
from typing import Any

import seaglass

from .actions import DARK_DISCOVERY_INDEX, END_TURN_INDEX, index_from_action

# -- Directions ----------------------------------------------------------------------------------


@dataclass(frozen=True)
class Direction:
    """A build: the cards that start it (lynchpins), the cards to buy once committed (`core`, plus
    every minion of `tribes` or with one of `tags`), and the late payoffs to level for."""

    name: str
    # The minion types the direction needs in the lobby (any one of them); empty: any lobby.
    tribes: tuple[str, ...]
    lynchpins: frozenset[int]
    core: frozenset[int] = frozenset()
    payoffs: frozenset[int] = frozenset()
    # Card tags (`seaglass.card_tags()`) whose minions fit a neutral direction.
    tags: tuple[str, ...] = ()
    # Whether buying and casting Tavern spells is part of the direction.
    spells: bool = False


def _d(name: str, tribes: tuple[str, ...], lynchpins, core=(), payoffs=(), tags=(), spells=False) -> Direction:
    return Direction(name, tribes, frozenset(lynchpins), frozenset(core), frozenset(payoffs), tuple(tags), spells)


# In order of preference when several lynchpins show up at once (roughly: easiest for a bot to play
# well first). Card ids are the engine's (`seaglass.catalog()`).
DIRECTIONS: list[Direction] = [
    _d("undead", ("undead",), lynchpins=(516, 517, 502, 530, 446, 209),
       core=(220, 225, 325, 209, 119), payoffs=(611, 622, 606)),
    _d("beasts", ("beast",), lynchpins=(552, 532, 524, 523, 545, 403, 411),
       core=(103, 212, 104, 429, 343), payoffs=(618, 607, 709)),
    _d("quilboar", ("quilboar",), lynchpins=(543, 540, 431, 406, 423),
       core=(314, 116, 224, 222), payoffs=(619, 631, 630)),
    _d("eat_the_tavern", ("demon", "elemental"), lynchpins=(527, 509, 510, 420, 455),
       core=(450, 402, 109), payoffs=(702, 625)),
    _d("elementals", ("elemental",), lynchpins=(531, 501, 417, 341, 448),
       core=(109, 456), payoffs=(628, 627, 711), spells=True),
    _d("pirates_gold", ("pirate",), lynchpins=(514, 515, 547, 426),
       core=(440, 439, 115), payoffs=(621, 615, 620, 701)),
    _d("pirates_bounties", ("pirate",), lynchpins=(539, 548, 404), payoffs=(621,), spells=True),
    _d("dragons", ("dragon",), lynchpins=(526, 528, 445, 231),
       core=(208, 303, 204, 329), payoffs=(614, 617, 604)),
    _d("mechs", ("mech",), lynchpins=(549, 541, 506, 416, 305),
       core=(202, 213, 223, 218), payoffs=(629, 601, 707)),
    _d("murlocs", ("murloc",), lynchpins=(544, 525, 546, 538, 436, 409),
       core=(112,), payoffs=(632, 603, 703)),
    _d("battlecries", (), lynchpins=(504,), core=(528, 436, 632), tags=("battlecry",)),
    _d("deathrattles", (), lynchpins=(551,), tags=("deathrattle",)),
    _d("end_of_turn", (), lynchpins=(512,), tags=("end_of_turn",)),
    _d("spells", (), lynchpins=(505, 542, 520, 519, 432),
       core=(330, 215, 306), payoffs=(602,), spells=True),
    _d("demons_damage", ("demon",), lynchpins=(550,), core=(105, 435), payoffs=(609,)),
    _d("aberrations", ("aberration",), lynchpins=(518, 533, 534, 413, 444),
       payoffs=(623, 613, 708), spells=True),
]

# Activate abilities worth using, and whether they take a friendly target (a board index).
_ACTIVATE_TARGETED = {121, 544, 626}  # Suspicious Prisonguard, Sewer Escapee, Tyrael
_ACTIVATE_UNTARGETED = {455, 509, 450, 631, 316}  # Jailer, Deft Deserter, Wrathguard, Geomant, Fruit Vendor

_TAGS_NAMES, _TAGS = seaglass.card_tags()
_TAG_INDEX = {n: i for i, n in enumerate(_TAGS_NAMES)}
_COUNTER_INDEX = {n: i for i, n in enumerate(seaglass.player_counter_names())}
_ALL_TRIBES = ("aberration", "beast", "demon", "dragon", "elemental", "mech", "murloc", "pirate",
               "quilboar", "undead")


def _has_tag(card_id: int, tag: str) -> bool:
    row = _TAGS.get(card_id)
    return bool(row) and row[_TAG_INDEX[tag]] > 0


def _stats(u: Any) -> int:
    return int(u.attack) + int(u.health)


# -- The bot -------------------------------------------------------------------------------------


@dataclass
class _Turn:
    """What the bot did this turn (reset at each new turn)."""

    turn: int = -1
    froze: bool = False
    sold: int = 0
    rolls: int = 0
    activated: set[int] = field(default_factory=set)


class NineKGamerBot:
    """`9kgamerbot` (see the module docstring). One instance per seat: it remembers its direction."""

    def __init__(
        self,
        level5_turn: int = 8,
        strong_health: int = 12,
        max_rolls: int = 20,
        late_levels: tuple[int, int] = (10, 13),
        tempo_levels: bool = False,
    ) -> None:
        # Up to Tier 4: level every turn it can afford to (False), or only when it can still buy a
        # minion after leveling, or on turn 2 (True: the usual curve, keeping a board).
        self.tempo_levels = tempo_levels
        self.level5_turn = level5_turn
        # Once committed, the turns by which it levels to Tier 5 and to Tier 6 even without feeling
        # strong (a Tier 4 board can't keep up late).
        self.late_levels = late_levels
        self.strong_health = strong_health
        self.max_rolls = max_rolls
        self.direction: Direction | None = None
        self._t = _Turn()

    # The lobby's minion types: `Seat` wraps a `LobbyGame` (which knows them); a plain
    # `TavernGame` gets every type.
    @staticmethod
    def _lobby_tribes(game: Any) -> tuple[str, ...]:
        lobby = getattr(game, "game", None)
        tribes = getattr(lobby, "tribes", None)
        return tuple(tribes) if tribes else _ALL_TRIBES

    def _open(self, d: Direction, tribes: tuple[str, ...]) -> bool:
        return not d.tribes or any(t in tribes for t in d.tribes)

    def _lynchpin_direction(self, card_id: int, tribes: tuple[str, ...]) -> Direction | None:
        for d in DIRECTIONS:
            if card_id in d.lynchpins and self._open(d, tribes):
                return d
        return None

    def _fits(self, u: Any, d: Direction) -> bool:
        cid = int(u.card_id)
        if cid in d.lynchpins or cid in d.core or cid in d.payoffs:
            return True
        if u.is_spell:
            return d.spells
        if d.tribes and any(t in d.tribes for t in u.tribes):
            return True
        return any(_has_tag(cid, tag) for tag in d.tags)

    def _value(self, u: Any, owned: Counter, tribes: tuple[str, ...], tier: int) -> float:
        """How much the bot wants minion `u` (in the shop, hand, board or a Discover)."""
        if u.is_spell:
            return 12.0 if self.direction is not None and self.direction.spells else 4.0
        cid = int(u.card_id)
        v = _stats(u) + 2.0 * int(u.tavern_tier)
        if not u.is_golden and owned[cid] >= 2:
            v += 25.0  # completes a triple
        d = self.direction
        if d is not None:
            if self._fits(u, d):
                v = 1.3 * v + 12.0
            if cid in d.lynchpins or cid in d.payoffs:
                v += 30.0
        elif tier >= 4 and self._lynchpin_direction(cid, tribes) is not None:
            v += 200.0
        return v

    def _key_card(self, u: Any, tribes: tuple[str, ...], tier: int) -> bool:
        """A lynchpin it is rolling for, or (committed) a card of its direction."""
        if u.is_spell:
            return False
        if self.direction is None:
            return tier >= 4 and self._lynchpin_direction(int(u.card_id), tribes) is not None
        return self._fits(u, self.direction)

    def _strong(self, obs: Any) -> bool:
        """A full board, Health to spare, and either a won last combat or plenty of Health."""
        won = obs.player_counters[_COUNTER_INDEX["last_combat_won"]] > 0
        # Duos: the team's shared Health (`DuosObservation.team_health`).
        health = (getattr(obs, "team_health", 0) or obs.health) + obs.armor
        return len(obs.board) >= 7 and health >= self.strong_health and (won or health >= 2 * self.strong_health)

    def act(self, game: Any) -> int:
        obs = game.observe()
        legal = game.legal_actions()
        if obs.turn != self._t.turn:
            self._t = _Turn(turn=obs.turn)
        tribes = self._lobby_tribes(game)
        tier = int(obs.tavern_tier)
        owned = Counter(int(u.card_id) for u in list(obs.board) + list(obs.hand) if not u.is_spell)
        by_kind: dict[str, list[Any]] = {}
        for a in legal:
            by_kind.setdefault(a.kind, []).append(a)
        idx = index_from_action

        # 0. Offers that block the turn: hero, trinket (the first), a Discover (the best).
        for kind in ("choose_hero", "choose_trinket"):
            if kind in by_kind:
                return idx(by_kind[kind][0])
        if obs.discover_pending and "choose_discover" in by_kind:
            opts = obs.discover_pending
            return idx(max(by_kind["choose_discover"],
                           key=lambda a: self._value(opts[a.args[0]], owned, tribes, tier)))

        # 1. Commit once it owns a lynchpin at Tier 4+.
        if self.direction is None and tier >= 4:
            for u in list(obs.board) + list(obs.hand):
                d = self._lynchpin_direction(int(u.card_id), tribes)
                if d is not None:
                    self.direction = d
                    break

        board_values = [self._value(u, owned, tribes, tier) for u in obs.board]

        # 2. Play from hand: minions to the end of the board (or Magnetized onto the best Mech when
        #    committed to Mechs), spells on the best friendly target.
        if "play" in by_kind:
            plays = by_kind["play"]
            for h, card in enumerate(obs.hand):
                mine = [a for a in plays if a.args[0] == h]
                if not mine:
                    continue
                if card.is_spell:
                    if len(obs.board) == 0:
                        return idx(mine[0])
                    return idx(max(mine, key=lambda a: board_values[a.args[1]]
                                   if a.args[1] < len(board_values) else 0.0))
                end = [a for a in mine if a.args[1] == len(obs.board)]
                if card.magnetic and self.direction is not None and self.direction.name == "mechs":
                    onto = [a for a in mine if a.args[1] < len(obs.board)]
                    if onto:
                        return idx(max(onto, key=lambda a: board_values[a.args[1]]))
                if end:
                    return idx(end[0])

        # 3. A full board with a better minion in hand: sell the weakest (not a lynchpin).
        hand_minions = [self._value(u, owned, tribes, tier) for u in obs.hand if not u.is_spell]
        if len(obs.board) >= 7 and hand_minions and "sell" in by_kind and self._t.sold < 4:
            weakest = min(range(len(obs.board)), key=lambda i: board_values[i])
            if max(hand_minions) > board_values[weakest] + 4.0:
                self._t.sold += 1
                return idx(next(a for a in by_kind["sell"] if a.args[0] == weakest))

        # 4. Activate the abilities worth using (once each per turn).
        for a in by_kind.get("activate", []):
            b, t = a.args
            cid = int(obs.board[b].card_id) if b < len(obs.board) else -1
            if (b, t) in self._t.activated:
                continue
            if cid in _ACTIVATE_UNTARGETED and t is None:
                self._t.activated.add((b, t))
                return idx(a)
        targeted = [a for a in by_kind.get("activate", [])
                    if a.args[0] < len(obs.board) and int(obs.board[a.args[0]].card_id) in _ACTIVATE_TARGETED
                    and a.args[1] is not None and a.args[1] < len(obs.board) and a.args[1] != a.args[0]
                    and a.args not in self._t.activated]
        if targeted:
            best = max(targeted, key=lambda a: board_values[a.args[1]])
            self._t.activated.add(best.args)
            return idx(best)

        # 5. Level: every turn up to Tier 4; to 5 if still uncommitted by `level5_turn`; once
        #    committed, to 5 and 6 when strong, or anyway by the `late_levels` turns.
        upgrade = seaglass.TavernAction.upgrade_tavern()
        if game.is_legal(upgrade):
            late = (tier == 4 and obs.turn >= self.late_levels[0]) or (tier == 5 and obs.turn >= self.late_levels[1])
            early = tier < 4 and (not self.tempo_levels or obs.turn == 2
                                  or obs.gold - obs.upgrade_cost >= obs.minion_cost)
            if (early
                    or (self.direction is None and tier == 4 and obs.turn >= self.level5_turn)
                    or (tier < 6 and tier >= 4 and (self._strong(obs) or late)
                        and (self.direction is not None or tier >= 5))):
                return idx(upgrade)

        # 6. Buy: the best minion if there is room (board + hand), or one clearly better than the
        #    weakest on a full board; spells when the direction wants them. The Dark Discovery
        #    button (a Discovered minion with a Dark Gift) beats a shop minion that isn't a lynchpin
        #    or, once committed, a direction card.
        room = len(obs.board) + sum(1 for u in obs.hand if not u.is_spell) < 7
        hand_room = len(obs.hand) < 10
        buys = by_kind.get("buy", [])
        minion_buys = [a for a in buys if not obs.shop[a.args[0]].is_spell]
        if obs.dark_discovery_usable and hand_room and room:
            wanted = [a for a in minion_buys if self._key_card(obs.shop[a.args[0]], tribes, tier)]
            if not wanted:
                return DARK_DISCOVERY_INDEX
        if minion_buys and hand_room:
            best = max(minion_buys, key=lambda a: self._value(obs.shop[a.args[0]], owned, tribes, tier))
            best_v = self._value(obs.shop[best.args[0]], owned, tribes, tier)
            weakest_v = min(board_values) if board_values else 0.0
            if room or best_v > weakest_v + 4.0:
                d = None if self.direction else self._lynchpin_direction(int(obs.shop[best.args[0]].card_id), tribes)
                if d is not None and tier >= 4:
                    self.direction = d
                return idx(best)
        spell_buys = [a for a in buys if obs.shop[a.args[0]].is_spell]
        if spell_buys and hand_room and self.direction is not None and self.direction.spells:
            return idx(spell_buys[0])

        # 7. Roll: for a lynchpin (uncommitted, Tier 4+, full board) or direction cards, or for
        #    more stats while there's room; always keeping enough Gold to buy what it finds.
        refresh = seaglass.TavernAction.refresh()
        can_roll = (game.is_legal(refresh) and self._t.rolls < self.max_rolls
                    and obs.gold >= obs.refresh_cost + obs.minion_cost)
        if can_roll and (len(obs.board) >= 7 or room):
            self._t.rolls += 1
            return idx(refresh)

        # 8. Freeze a shop with a lynchpin or a direction card it couldn't afford.
        if not self._t.froze and not obs.is_frozen and "toggle_freeze" in by_kind:
            wanted = [u for u in obs.shop if not u.is_spell and (
                (self.direction is None and tier >= 4 and self._lynchpin_direction(int(u.card_id), tribes))
                or (self.direction is not None and self._fits(u, self.direction)
                    and int(u.card_id) in self.direction.lynchpins | self.direction.payoffs))]
            if wanted:
                self._t.froze = True
                return idx(by_kind["toggle_freeze"][0])

        # 9. Leftover Gold, once committed: a spell (any). Then End Turn.
        if spell_buys and hand_room and self.direction is not None and len(obs.board) >= 1:
            return idx(spell_buys[0])
        if game.is_legal(seaglass.TavernAction.end_turn()):
            return END_TURN_INDEX
        return idx(legal[0]) if legal else END_TURN_INDEX
