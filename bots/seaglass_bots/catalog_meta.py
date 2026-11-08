"""Dynamic card metadata and vocabulary indexed directly from Rust (`seaglass.card_tags`)."""

from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
from typing import Dict, List, Tuple

import seaglass


@dataclass(frozen=True)
class CardProfile:
    card_id: int
    vocab_idx: int
    is_minion: bool
    is_temporary: bool
    is_intrinsic_golden: bool
    tier: int
    tribes: Tuple[str, ...]
    has_battlecry: bool
    has_deathrattle: bool
    has_rally: bool
    has_start_of_combat: bool
    has_end_of_turn: bool
    has_turn_start: bool
    has_avenge: bool
    has_cleave: bool
    has_poisonous: bool
    has_activate: bool
    has_on_sell: bool
    is_scaling_engine: bool
    is_economy_generator: bool
    is_combat_summoner: bool
    is_combat_observer: bool
    triggers_on_play: bool
    triggers_on_spell: bool
    triggers_on_gold_or_buy: bool
    triggers_on_magnetize: bool
    triggers_on_battlecry: bool
    engine_weight: float
    raw_tags: Tuple[float, ...]


class CardMetaDB:
    """Indexes engine-authored hooks, templates, and tag vectors without hardcoding card IDs."""

    def __init__(self, catalog_name: str = "full") -> None:
        tag_names, tag_table = seaglass.card_tags()
        self.tag_names: List[str] = list(tag_names)
        self.tag_dim: int = len(self.tag_names)
        self.tag_index: Dict[str, int] = {name: i for i, name in enumerate(self.tag_names)}

        self.counter_names: List[str] = list(seaglass.player_counter_names())
        self.counter_index: Dict[str, int] = {
            name: i for i, name in enumerate(self.counter_names)
        }

        self.templates_by_id: Dict[int, seaglass.CardTemplate] = {}
        self.templates_by_tier: Dict[int, List[seaglass.CardTemplate]] = {
            t: [] for t in range(1, 8)
        }
        for tmpl in seaglass.catalog(catalog_name):
            self.templates_by_id[tmpl.card_id] = tmpl
            self.templates_by_tier.setdefault(tmpl.tavern_tier, []).append(tmpl)

        # Deterministic 1-based vocabulary (0 reserved for PAD / unknown card_id).
        sorted_cids = sorted(tag_table.keys())
        self.vocab_ids: List[int] = [0] + sorted_cids
        self.id_to_vocab: Dict[int, int] = {cid: idx + 1 for idx, cid in enumerate(sorted_cids)}
        self.vocab_size: int = len(self.vocab_ids)

        self.tag_matrix: List[List[float]] = [[0.0] * self.tag_dim]
        self.profiles: Dict[int, CardProfile] = {}
        for cid in sorted_cids:
            row = tag_table[cid]
            v_idx = self.id_to_vocab[cid]
            self.tag_matrix.append(list(row))
            self.profiles[cid] = self._build_profile(cid, v_idx, row)

    def vocab_index(self, card_id: int) -> int:
        return self.id_to_vocab.get(card_id, 0)

    def _has(self, row: List[float], *names: str) -> bool:
        for name in names:
            idx = self.tag_index.get(name)
            if idx is not None and row[idx] > 0.5:
                return True
        return False

    def _build_profile(self, cid: int, vocab_idx: int, row: List[float]) -> CardProfile:
        is_minion = self._has(row, "minion")
        is_temporary = self._has(row, "flag_temporary")
        is_intrinsic_golden = self._has(row, "intrinsic_golden")
        tier = 1
        for t in range(1, 8):
            if self._has(row, f"tier_{t}"):
                tier = t
                break

        tribes: List[str] = []
        for tag_name, idx in self.tag_index.items():
            if tag_name.startswith("tribe_") and row[idx] > 0.5:
                tribes.append(tag_name[len("tribe_") :])

        has_battlecry = self._has(row, "battlecry")
        has_deathrattle = self._has(row, "deathrattle")
        has_rally = self._has(row, "rally")
        has_soc = self._has(row, "start_of_combat", "start_of_combat_action")
        has_eot = self._has(row, "end_of_turn")
        has_turn_start = self._has(row, "turn_start")
        has_avenge = self._has(row, "flag_avenge", "avenge")
        has_cleave = self._has(row, "flag_cleave", "flag_excess_damage_to_neighbors")
        has_poisonous = self._has(row, "flag_poisonous", "venomous")
        has_activate = self._has(row, "activate")
        has_on_sell = self._has(row, "sell")

        triggers_on_play = self._has(
            row, "after_friendly_play", "first_play_or_magnetize", "card_gained", "card_added_to_hand"
        )
        triggers_on_spell = self._has(
            row, "spell_cast", "after_spell_cast", "after_targeted_spell", "spell_aura", "combat_spell_cast"
        )
        triggers_on_gold_or_buy = self._has(
            row, "gold_spent", "after_buy", "minion_bought", "after_shop_refresh"
        )
        triggers_on_magnetize = self._has(row, "after_friendly_magnetize")
        triggers_on_battlecry = self._has(row, "after_friendly_battlecry")

        is_scaling = (
            has_eot
            or has_avenge
            or triggers_on_play
            or triggers_on_spell
            or triggers_on_gold_or_buy
            or triggers_on_magnetize
            or triggers_on_battlecry
            or self._has(row, "after_friendly_sell", "passive", "hero_damaged", "after_hero_damage")
        )
        is_economy = self._has(
            row,
            "turn_start",
            "on_sell",
            "sell",
            "activate",
            "discover",
            "flag_free_pass",
            "flag_health_refreshes",
        )
        is_summoner = has_deathrattle or self._has(row, "reborn")
        is_combat_obs = self._has(
            row,
            "friendly_death",
            "friendly_summon",
            "friendly_attack",
            "after_friendly_rally",
            "friendly_divine_shield_lost",
            "after_friendly_reborn",
            "combat_spell_cast",
        )

        weight = 0.0
        if is_scaling:
            weight += 2.0 + 0.75 * tier
        if has_eot:
            weight += 1.5 + 0.45 * tier
        if has_avenge:
            weight += 1.6 + 0.40 * tier
        if is_combat_obs:
            weight += 1.4 + 0.45 * tier
        if is_economy:
            weight += 1.0 + 0.20 * tier
        if has_cleave:
            weight += 2.6

        return CardProfile(
            card_id=cid,
            vocab_idx=vocab_idx,
            is_minion=is_minion,
            is_temporary=is_temporary,
            is_intrinsic_golden=is_intrinsic_golden,
            tier=tier,
            tribes=tuple(tribes),
            has_battlecry=has_battlecry,
            has_deathrattle=has_deathrattle,
            has_rally=has_rally,
            has_start_of_combat=has_soc,
            has_end_of_turn=has_eot,
            has_turn_start=has_turn_start,
            has_avenge=has_avenge,
            has_cleave=has_cleave,
            has_poisonous=has_poisonous,
            has_activate=has_activate,
            has_on_sell=has_on_sell,
            is_scaling_engine=is_scaling,
            is_economy_generator=is_economy,
            is_combat_summoner=is_summoner,
            is_combat_observer=is_combat_obs,
            triggers_on_play=triggers_on_play,
            triggers_on_spell=triggers_on_spell,
            triggers_on_gold_or_buy=triggers_on_gold_or_buy,
            triggers_on_magnetize=triggers_on_magnetize,
            triggers_on_battlecry=triggers_on_battlecry,
            engine_weight=weight,
            raw_tags=tuple(row),
        )

    def profile(self, card_id: int) -> CardProfile | None:
        return self.profiles.get(card_id)

    def card_name(self, card_id: int) -> str:
        tmpl = self.templates_by_id.get(card_id)
        if tmpl is not None:
            return tmpl.name
        return f"Card#{card_id}"

    def counter(self, obs: seaglass.TavernObservation, name: str) -> int:
        idx = self.counter_index.get(name)
        if idx is None or idx >= len(obs.player_counters):
            return 0
        return obs.player_counters[idx]


@lru_cache(maxsize=4)
def get_meta_db(catalog_name: str = "full") -> CardMetaDB:
    return CardMetaDB(catalog_name)
