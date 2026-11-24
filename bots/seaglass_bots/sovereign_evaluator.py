"""Sovereign Evaluator: Positioned 1-Turn Physics, 5-Tribe Conditioning, Bellman t+1 Shop & Pure-8p-FFA Net."""

from __future__ import annotations

import math
from typing import Dict, FrozenSet, List, Optional, Sequence, Tuple

import seaglass

from .apex_knowledge import ApexKnowledge, get_apex_knowledge
from .sovereign_belief import SovereignOpponentParticles
from .sovereign_knowledge import SovereignKnowledge, get_sovereign_knowledge
from .sovereign_model import SovereignNumpyPredictor, get_sovereign_predictor
from .sovereign_positioning import SovereignPositioner

# Calibrated 8-Player FFA Tavern Tier curve (crisp tempo on Turns 1-5 + disciplined Tier 4->5->6 scaling).
_SOV_TIER_CURVE = (0.0, 0.0, 9.6, 18.0, 29.2, 48.2, 66.5, 80.0)
_BASE_UPGRADE_COST = (0, 5, 7, 8, 10, 10, 0, 0)

_TRIBE_TAVERN_COUNTER_PAIRS = (
    ("aberration", "aberration_tavern_atk", "aberration_tavern_hp"),
    ("beast", "beast_tavern_atk", "beast_tavern_hp"),
    ("demon", "demon_tavern_atk", "demon_tavern_hp"),
    ("dragon", "dragon_tavern_atk", "dragon_tavern_hp"),
    ("elemental", "elemental_tavern_atk", "elemental_tavern_hp"),
    ("mech", "mech_tavern_atk", "mech_tavern_hp"),
    ("murloc", "murloc_tavern_atk", "murloc_tavern_hp"),
    ("pirate", "pirate_tavern_atk", "pirate_tavern_hp"),
    ("quilboar", "quilboar_tavern_atk", "quilboar_tavern_hp"),
    ("undead", "undead_tavern_atk", "undead_tavern_hp"),
    ("naga", "naga_tavern_atk", "naga_tavern_hp"),
)


def _concave_stat_value(atk: float, hp: float) -> float:
    a = atk if atk > 0.0 else 0.0
    h = hp if hp > 0.0 else 0.0
    ca = a if a <= 80.0 else (80.0 + 22.5 * math.sqrt(a - 80.0))
    ch = h if h <= 80.0 else (80.0 + 22.5 * math.sqrt(h - 80.0))
    return 0.49 * ca + 0.47 * ch


class SovereignEvaluator:
    """
    Upgraded State & Physics Evaluator for `SovereignBot`:
      1. Runs `<0.05ms` `SovereignPositioner.fast_reposition_game` BEFORE `combat_game.end_turn()`
         inside `evaluate_turn_end`, curing ApexBot's unpositioned-search blindness!
      2. Conditions Hero selection, Neutral multipliers (`Brann`, `Titus`, `Drakkari`, `Young Murk-Eye`),
         and Trinkets on the active 5-tribe lobby (`ctx.tribes`) + empirical 8-ApexBot telemetry!
      3. Evaluates `t+1` Start-of-Turn shop opportunities (`in_dist_sot=True`) for Bellman-consistent
         leveling and freeze pricing.
      4. Fuses pure-8p-FFA `SovereignNumpyPredictor` (~0.045ms BLAS) across all stages including Top-2 finals.
    """

    def __init__(
        self,
        catalog_name: str = "full",
        use_physics_rollout: bool = True,
        use_neural_value: bool = True,
        neural_weight: float = 0.20,
        weights_path: Optional[str] = None,
    ) -> None:
        self.catalog_name = catalog_name
        self.sov_kb: SovereignKnowledge = get_sovereign_knowledge(catalog_name)
        self.kb: ApexKnowledge = self.sov_kb.base
        self.meta = self.kb.meta
        self.use_physics_rollout = use_physics_rollout
        self.use_neural_value = use_neural_value
        self.neural_weight = neural_weight
        self.predictor: Optional[SovereignNumpyPredictor] = (
            get_sovereign_predictor(catalog_name=catalog_name, weights_path=weights_path)
            if use_neural_value
            else None
        )
        self.positioner = SovereignPositioner(
            catalog_name=catalog_name, max_candidates=56, evaluator=self
        )

        # Pre-bind card & trinket IDs
        self._brann_id = self.kb.name_to_id.get("Brann Bronzebeard", -1)
        self._drakkari_id = self.kb.name_to_id.get("Drakkari Enchanter", -1)
        self._titus_id = self.kb.name_to_id.get("Titus Rivendare", -1)
        self._murkeye_id = self.kb.young_murk_eye_id
        self._poet_id = self.kb.persistent_poet_id
        self._runic_arcanist_id = self.kb.name_to_id.get("Runic Arcanist", -1)
        self._tarecgosa_id = self.kb.name_to_id.get("Tarecgosa", -1)
        self._leeroy_id = self.kb.leeroy_id
        self._morgl_id = self.kb.morgl_id
        self._sindorei_id = self.kb.sindorei_id
        self._amalgam_id = self.kb.gatekeeper_amalgam_id
        self._boom_id = self.kb.boom_in_a_box_id
        self._tyrael_id = self.kb.tyrael_id
        self._sanders_id = self.kb.captain_sanders_id
        self._last_standing_id = self.kb.last_standing_id
        self._candle_id = self.kb.guiding_candle_id
        self._upper_hand_id = self.kb.upper_hand_id
        self._gauntlet_ids = self.kb.weighted_gauntlet_ids
        self._eot_trinket_ids = self.kb.eot_trinket_ids

        self._dragon_combat_buffer_ids: FrozenSet[int] = frozenset(
            cid
            for name in (
                "Electric Synthesizer",
                "Amber Guardian",
                "Runic Arcanist",
                "Hopebringer",
                "Crimson Vindicator",
                "Stalwart Kodo",
                "Goldrinn, the Great Wolf",
            )
            if (cid := self.kb.name_to_id.get(name)) is not None
        )
        self._dragon_combat_trinket_ids: FrozenSet[int] = frozenset(
            tid
            for tid, (tname, _, _, _) in self.kb.trinket_info.items()
            if tname in ("Training Certificate", "Bronze Timepiece", "Holy Mallet")
        )

        self._hand_trinket_ids: FrozenSet[int] = frozenset(
            tid
            for tid, (tname, _, _, _) in self.kb.trinket_info.items()
            if tname in ("Nerglish Phrasebook", "Dramaloc Sticker", "Bluegill Flippers", "Flighty Portrait")
        )
        self._voone_hero_ids: FrozenSet[int] = frozenset(
            hid for hid, hname in self.kb.hero_names.items() if hname == "Rock Master Voone"
        )
        self._trinket_prior_by_id: Dict[int, float] = {
            tid: self.sov_kb.trinket_prior_score(tname)
            for tid, (tname, _, _, _) in self.kb.trinket_info.items()
        }

        self._support_self_has_tribe: Dict[int, bool] = {}
        for cid, req_tr in self.kb.required_ally_by_id.items():
            prof = self.meta.profile(cid)
            has_tr = bool(prof and (req_tr in prof.tribes or "all" in prof.tribes))
            self._support_self_has_tribe[cid] = has_tr
        for cid, syn_tr in self.kb.tribal_synergy_by_id.items():
            prof = self.meta.profile(cid)
            has_tr = bool(prof and (syn_tr in prof.tribes or "all" in prof.tribes))
            self._support_self_has_tribe[cid] = has_tr

        cidx = self.meta.counter_index
        self._c_max_gold = cidx.get("base_max_gold_bonus", -1)
        self._c_gem_atk = cidx.get("blood_gem_bonus_atk", -1)
        self._c_gem_hp = cidx.get("blood_gem_bonus_hp", -1)
        self._c_sp_atk = cidx.get("spell_bonus_atk", -1)
        self._c_sp_hp = cidx.get("spell_bonus_hp", -1)
        self._c_el_atk = cidx.get("tavern_elemental_atk", -1)
        self._c_el_hp = cidx.get("tavern_elemental_hp", -1)
        self._c_all_atk = cidx.get("tavern_all_atk", -1)
        self._c_all_hp = cidx.get("tavern_all_hp", -1)
        self._c_ud_atk = cidx.get("undead_bonus_attack", -1)
        self._c_ud_hp = cidx.get("undead_bonus_health", -1)
        self._c_ref_gems = cidx.get("refresh_blood_gems", -1)
        self._c_ref_buffs = cidx.get("refresh_buffs", -1)
        self._tribe_counter_idx: Tuple[Tuple[str, int, int], ...] = tuple(
            (tr, cidx[ca], cidx[ch])
            for tr, ca, ch in _TRIBE_TAVERN_COUNTER_PAIRS
            if ca in cidx and ch in cidx
        )

    def refresh_knowledge_priors(self) -> None:
        """Refresh cached trinket priors after updating `EMPIRICAL_TRINKET_DELTAS`."""
        self._trinket_prior_by_id = {
            tid: self.sov_kb.trinket_prior_score(tname)
            for tid, (tname, _, _, _) in self.kb.trinket_info.items()
        }

    @property
    def has_neural_model(self) -> bool:
        return self.predictor is not None and self.predictor.is_ready

    @staticmethod
    def effective_hp(obs: seaglass.TavernObservation) -> float:
        hp = getattr(obs, "team_health", obs.health)
        arm = getattr(obs, "team_armor", obs.armor)
        return float((hp if hp > 0 else 0) + (arm if arm > 0 else 0))

    @staticmethod
    def cvar_survival_utility(
        hp_after: float,
        alive_units: int,
        lobby_hps: Sequence[int],
        damage_cap: Optional[int],
    ) -> float:
        if hp_after <= 0.0:
            overkill_pen = 1.50 * max(-80.0, hp_after)
            return -200.0 - 14.5 * max(0, alive_units - 2) + overkill_pen

        base = 30.5 * math.log1p(hp_after / 7.0)
        next_danger = float(damage_cap) if damage_cap is not None else 18.0
        if hp_after <= next_danger:
            deficit = (next_danger + 1.0 - hp_after) / max(5.0, next_danger)
            base -= 14.5 * (deficit ** 1.4)

        if lobby_hps:
            s_hps = sorted(lobby_hps)
            median_hp = float(s_hps[len(s_hps) // 2])
            rel = (hp_after - median_hp) / max(10.0, median_hp)
            base += 5.2 * math.tanh(rel)
        return base

    def effective_engine_bonus(
        self,
        card_id: int,
        turn: int,
        tribe_counts: Dict[str, int],
        on_board: bool = False,
        board_units: Optional[Sequence[seaglass.Unit]] = None,
        is_golden: bool = False,
        obs_trinkets: Optional[Sequence[int]] = None,
        lobby_tribes: Optional[Sequence[str]] = None,
    ) -> float:
        w = self.kb.engine_bonus_by_id.get(card_id, 0.0)
        if card_id == self._runic_arcanist_id:
            has_drag_ally = (
                tribe_counts.get("dragon", 0) + tribe_counts.get("all", 0)
                - (1 if on_board else 0)
            ) >= 1 or (
                board_units is not None
                and any(bu.card_id in (self._poet_id, self._tarecgosa_id) for bu in board_units)
            )
            if has_drag_ally:
                return 11.5
            return 8.0 if turn <= 10 else 4.5
        if w <= 0.0:
            return 0.0
        if card_id == self._poet_id:
            has_dragon_buf = (
                board_units is not None
                and any(bu.card_id in self._dragon_combat_buffer_ids for bu in board_units)
            ) or (
                obs_trinkets is not None
                and any(tid in self._dragon_combat_trinket_ids for tid in obs_trinkets)
            )
            other_dragons = (
                tribe_counts.get("dragon", 0) + tribe_counts.get("all", 0)
                - (1 if on_board else 0)
            )
            if not has_dragon_buf:
                if turn >= 9:
                    return 0.0
                return 4.0 if on_board else (5.5 if other_dragons >= 1 else 0.0)
            if other_dragons <= 0:
                return 0.0 if turn >= 9 else 4.5
            if on_board and not is_golden and board_units is not None and turn >= 11:
                plain_poets = sum(
                    1 for bu in board_units if bu.card_id == self._poet_id and not bu.is_golden
                )
                if plain_poets >= 2:
                    return 8.5
            return 11.8
        if card_id == self.kb.soul_rewinder_id and turn <= 10 and tribe_counts.get("demon", 0) >= 2:
            if on_board and board_units is not None:
                if sum(1 for bu in board_units if bu.card_id == self.kb.soul_rewinder_id) == 1:
                    return 10.8
            elif board_units is None or not any(bu.card_id == self.kb.soul_rewinder_id for bu in board_units):
                return 10.4
        if on_board:
            if card_id == self._sanders_id:
                return 0.0
            if card_id == self._tyrael_id and board_units is not None:
                cap = 195 if is_golden else 95
                already_set = sum(
                    1 for bu in board_units if bu.card_id != self._tyrael_id and (bu.attack + bu.health) >= cap
                )
                if already_set >= 5:
                    return 0.0

        # 5-Tribe Lobby Conditioning on neutral multipliers (`Brann`, `Titus`, `Drakkari`, `Young Murk-Eye`)
        tribe_mult = (
            self.sov_kb.neutral_engine_tribe_multiplier(card_id, lobby_tribes)
            if lobby_tribes
            else 1.0
        )

        if board_units is not None and turn >= 9:
            profiles = self.meta.profiles
            if card_id == self._murkeye_id:
                bc_allies = sum(
                    1
                    for bu in board_units
                    if bu.card_id != self._murkeye_id
                    and (p := profiles.get(bu.card_id)) is not None
                    and p.has_battlecry
                )
                if bc_allies == 0:
                    if on_board or turn >= 10:
                        return 0.0
                    return 0.35 * w * tribe_mult
            elif card_id == self._drakkari_id and obs_trinkets is not None:
                has_eot_trinket = any(tid in self._eot_trinket_ids for tid in obs_trinkets)
                if not has_eot_trinket:
                    eot_allies = sum(
                        1
                        for bu in board_units
                        if bu.card_id != self._drakkari_id
                        and (
                            bu.end_of_turn_stats > 0
                            or ((p := profiles.get(bu.card_id)) is not None and p.has_end_of_turn)
                        )
                    )
                    if eot_allies == 0:
                        if on_board or turn >= 10:
                            return 0.0
                        return 0.35 * w * tribe_mult
            elif card_id == self._titus_id:
                dr_allies = sum(
                    1
                    for bu in board_units
                    if bu.card_id != self._titus_id
                    and (
                        bu.extra_deathrattles > 0
                        or ((p := profiles.get(bu.card_id)) is not None and p.has_deathrattle)
                    )
                )
                if dr_allies == 0:
                    if on_board or turn >= 10:
                        return 0.0
                    return 0.35 * w * tribe_mult
            elif card_id == self._brann_id and on_board and turn >= 10:
                bc_allies = sum(
                    1
                    for bu in board_units
                    if bu.card_id != self._brann_id
                    and (p := profiles.get(bu.card_id)) is not None
                    and p.has_battlecry
                )
                if bc_allies == 0 and tribe_counts.get("murloc", 0) < 2 and tribe_counts.get("dragon", 0) < 3:
                    return 7.0 * tribe_mult

        req_tr = self.kb.required_ally_by_id.get(card_id)
        if req_tr is not None:
            allies = tribe_counts.get(req_tr, 0) + tribe_counts.get("all", 0)
            if on_board and self._support_self_has_tribe.get(card_id, False):
                other_allies = allies - 1 if allies > 0 else 0
            else:
                other_allies = allies
            if other_allies == 0:
                return 0.0
            return w

        if card_id == self.kb.banana_slamma_id and board_units is not None and turn >= 8:
            profiles = self.meta.profiles
            beast_summoners = sum(
                1
                for bu in board_units
                if bu.card_id != self.kb.banana_slamma_id
                and ("beast" in bu.tribes or "all" in bu.tribes)
                and (
                    bu.reborn
                    or bu.extra_deathrattles > 0
                    or ((p := profiles.get(bu.card_id)) is not None and p.is_combat_summoner)
                )
            )
            if beast_summoners == 0:
                return 0.0 if (on_board or turn >= 9) else 4.5

        syn_tr = self.kb.tribal_synergy_by_id.get(card_id)
        if syn_tr is not None:
            allies = tribe_counts.get(syn_tr, 0) + tribe_counts.get("all", 0)
            if on_board and self._support_self_has_tribe.get(card_id, False):
                other_allies = allies - 1 if allies > 0 else 0
            else:
                other_allies = allies
            if other_allies >= 1:
                boosted = w + 0.52 * (other_allies if other_allies < 4 else 4)
                if w < 12.0 and boosted >= 11.8:
                    boosted = 11.8
                if (
                    on_board
                    and turn >= 9
                    and other_allies == 1
                    and max(tribe_counts.values(), default=0) >= 4
                    and tribe_counts.get(syn_tr, 0) <= 2
                ):
                    return min(boosted, 8.8)
                return boosted
            if turn <= 8:
                return 0.78 * w if w >= 12.0 else 0.45 * w
            if turn == 9:
                return 0.35 * w if w >= 12.5 else 0.0
            return 0.0

        return w * tribe_mult

    def has_hand_buff_synergy(self, obs: seaglass.TavernObservation) -> bool:
        board_ids = self.kb.hand_synergy_board_ids
        for u in obs.board:
            if u.card_id in board_ids:
                return True
        if obs.hero_id in self._voone_hero_ids:
            return True
        h_trinkets = self._hand_trinket_ids
        for tid in obs.trinkets:
            if tid in h_trinkets:
                return True
        recv_ids = self.kb.hand_buff_receiver_ids
        for c in obs.hand:
            if c.card_id in recv_ids:
                return True
        return False

    def unit_value(
        self,
        u: seaglass.Unit,
        turn: int,
        tribe_counts: Dict[str, int],
        enemy_has_giant: bool = False,
        enemy_has_summoners: bool = False,
        on_board: bool = True,
        colossus_count: int = 0,
        enemy_has_ds: bool = False,
        board_units: Optional[Sequence[seaglass.Unit]] = None,
        obs_trinkets: Optional[Sequence[int]] = None,
        lobby_tribes: Optional[Sequence[str]] = None,
    ) -> float:
        atk = float(u.attack if u.attack > 0 else 0)
        hp = float(u.health if u.health > 0 else 0)
        val = _concave_stat_value(atk, hp) + 0.95 * u.tavern_tier

        if u.divine_shield:
            val += 3.2 + 0.36 * (atk if atk < 90.0 else 90.0)
        if u.reborn:
            val += 2.0 + 0.25 * u.tavern_tier
        if u.venomous:
            ven_base = 7.6 + 0.52 * (turn if turn < 15 else 15) + (6.5 if u.divine_shield else 0.0)
            if u.card_id == self._morgl_id:
                ven_base += 18.5 + (22.0 if (enemy_has_giant or colossus_count >= 1) else 0.0)
            if enemy_has_giant:
                ven_base += 10.5
            if colossus_count >= 2:
                ven_base += 36.0 + 13.0 * min(4, colossus_count)
                if u.reborn or u.divine_shield:
                    ven_base += 24.0
                if u.reborn and u.divine_shield:
                    ven_base += 18.0
            elif (
                not enemy_has_giant
                and colossus_count == 0
                and not u.divine_shield
                and u.card_id != self._morgl_id
                and board_units is not None
            ):
                existing_ven = sum(1 for bu in board_units if bu.venomous)
                if (not on_board and existing_ven >= 1) or (on_board and existing_ven >= 2):
                    ven_base *= 0.42
            val += ven_base
        if u.card_id == self._leeroy_id:
            lr_base = 7.6 + 0.48 * (turn if turn < 15 else 15) + (9.5 if enemy_has_giant else 0.0)
            if colossus_count >= 2:
                lr_base += 34.0 + 12.0 * min(4, colossus_count) + (20.0 if u.reborn else 0.0)
            val += lr_base
        if u.card_id == self._sindorei_id and (
            (colossus_count >= 2 and enemy_has_ds) or colossus_count >= 4
        ):
            val += 20.0
        prof = self.meta.profiles.get(u.card_id)
        if u.windfury:
            val += 1.0 + 0.14 * (atk if atk < 80.0 else 80.0)
        if u.taunt:
            val += 0.65 + 0.04 * (hp if hp < 80.0 else 80.0)
        if u.is_golden:
            if prof is not None and prof.is_intrinsic_golden:
                val += 1.2
            else:
                val += 3.6 + 0.85 * u.tavern_tier

        if u.end_of_turn_stats:
            val += 2.0 * u.end_of_turn_stats
        if u.extra_deathrattles:
            val += 1.8 * u.extra_deathrattles
        if u.dark_gifts:
            dg_w = self.kb.dark_gift_weights
            for dg in u.dark_gifts:
                val += 0.55 * dg_w.get(dg, 4.0)

        raw_h = (16.5 - turn) / 9.0
        horizon = 0.22 if raw_h < 0.22 else (1.40 if raw_h > 1.40 else raw_h)
        sov_eng = self.effective_engine_bonus(
            u.card_id,
            turn,
            tribe_counts,
            on_board=on_board,
            board_units=board_units,
            is_golden=u.is_golden,
            obs_trinkets=obs_trinkets,
            lobby_tribes=lobby_tribes,
        )
        if sov_eng > 0.0:
            gold_mult = 1.75 if u.is_golden else 1.0
            val += 0.86 * sov_eng * horizon * gold_mult
        if u.card_id == self._last_standing_id:
            if u.windfury:
                val += 18.0
            if u.divine_shield:
                val += 12.0

        if prof is not None:
            val += 0.70 * prof.engine_weight * horizon * (1.55 if u.is_golden else 1.0)
            if prof.has_cleave:
                val += 0.43 * (atk if atk < 90.0 else 90.0) + (3.5 if enemy_has_summoners else 0.0)
            if (
                turn >= 7
                and u.tavern_tier <= 2
                and sov_eng <= 0.0
                and not prof.is_scaling_engine
                and (atk + hp) < 22
                and not u.venomous
            ):
                val -= 0.75 * (turn - 6)

            for tr in prof.tribes:
                cnt = tribe_counts.get(tr, 0)
                if cnt >= 2:
                    c_cap = cnt - 1 if cnt < 6 else 5
                    val += 0.52 * c_cap
        return val

    def unit_keep_priority(
        self,
        u: seaglass.Unit,
        turn: int,
        tribe_counts: Dict[str, int],
        scam_count: int = 0,
        particles: Optional[SovereignOpponentParticles] = None,
        board_units: Optional[Sequence[seaglass.Unit]] = None,
        obs_trinkets: Optional[Sequence[int]] = None,
        lobby_tribes: Optional[Sequence[str]] = None,
    ) -> float:
        enemy_giant = particles.has_giant_unit if particles is not None else False
        enemy_sum = particles.has_summoners if particles is not None else False
        eff_col = (
            max(particles.colossus_count, particles.lobby_colossus_count)
            if particles is not None
            else 0
        )
        enemy_ds = particles.has_divine_shields if particles is not None else False
        score = self.unit_value(
            u,
            turn,
            tribe_counts,
            enemy_has_giant=enemy_giant,
            enemy_has_summoners=enemy_sum,
            on_board=True,
            colossus_count=eff_col,
            enemy_has_ds=enemy_ds,
            board_units=board_units,
            obs_trinkets=obs_trinkets,
            lobby_tribes=lobby_tribes,
        )
        sov_eng = self.effective_engine_bonus(
            u.card_id,
            turn,
            tribe_counts,
            on_board=True,
            board_units=board_units,
            is_golden=u.is_golden,
            obs_trinkets=obs_trinkets,
            lobby_tribes=lobby_tribes,
        )
        if sov_eng >= 10.0:
            h_keep = (16.5 - turn) / 9.0
            h_clamp = 0.45 if h_keep < 0.45 else (1.30 if h_keep > 1.30 else h_keep)
            score += (30.0 if sov_eng >= 12.0 else 19.0) * h_clamp

        max_scam_keep = 5 if eff_col >= 3 else (3 if (eff_col >= 1 or enemy_giant) else 1)
        if turn >= 9 and (u.venomous or u.card_id in (self._leeroy_id, self._morgl_id)) and scam_count <= max_scam_keep:
            if eff_col >= 3:
                score += 48.0
            elif eff_col >= 1 or enemy_giant:
                score += 28.0
            else:
                score += 18.0 if (u.divine_shield or u.card_id == self._morgl_id) else 8.0

        if u.card_id in self.kb.true_on_sell_plain_ids and sov_eng < 8.0 and not u.venomous:
            score -= 8.5
        elif u.card_id in self.kb.golden_only_sell_ids and u.is_golden and not u.venomous:
            score -= 18.0
        return score

    def _sot_shop_opportunity_value(
        self,
        obs: seaglass.TavernObservation,
        all_copies: Dict[int, int],
        tribe_counts: Dict[str, int],
        lobby_tribes: Optional[Sequence[str]],
    ) -> float:
        """
        Bellman-consistent Start-of-Turn (`t + 1`) shop opportunity score:
          Rewards leveling into higher-tier `t+1` shops AND rewards freezing high-value triples/carries!
        """
        if not obs.shop or obs.gold < 1:
            return 0.0
        turn = obs.turn
        tier = obs.tavern_tier
        board = obs.board
        obs_trinkets = obs.trinkets
        gains: List[float] = []
        for s in obs.shop:
            if s.is_spell:
                if s.spell_cost <= obs.gold and s.tavern_tier >= max(3, tier):
                    gains.append(0.8 + 0.25 * s.tavern_tier)
                continue
            if obs.gold < obs.minion_cost:
                continue
            eng_b = self.effective_engine_bonus(
                s.card_id,
                turn,
                tribe_counts,
                on_board=False,
                board_units=board,
                obs_trinkets=obs_trinkets,
                lobby_tribes=lobby_tribes,
            )
            is_triple = (not s.is_golden) and all_copies.get(s.card_id, 0) >= 2
            val = 0.0
            if is_triple and (s.tavern_tier >= max(1, tier - 2) or turn <= 7 or eng_b >= 8.0):
                val = 3.8 + 0.45 * min(6, tier + 1)
            elif eng_b >= 10.0:
                val = 1.4 + 0.22 * eng_b
            elif len(board) < 7 and s.tavern_tier >= tier:
                val = 0.9 + 0.30 * s.tavern_tier
            if val > 0.0:
                gains.append(val)
        if not gains:
            return 0.0
        gains.sort(reverse=True)
        top_sum = gains[0] + (0.45 * gains[1] if len(gains) > 1 and obs.gold >= 2 * obs.minion_cost else 0.0)
        return min(6.5, top_sum)

    def macro_and_economy_value(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = True,
        particles: Optional[SovereignOpponentParticles] = None,
        in_dist_sot: bool = False,
        sot_shop_is_real: bool = False,
    ) -> float:
        turn = obs.turn if obs.turn > 1 else 1
        hp_total = self.effective_hp(obs)
        if hp_total < 1.0:
            hp_total = 1.0
        dmg_cap = ctx.damage_cap if ctx is not None else None
        alive_units = ctx.alive_units if ctx is not None else 8
        lobby_tribes = ctx.tribes if ctx is not None else None
        safe_denom = float(dmg_cap + 6) if dmg_cap is not None else 25.0
        if safe_denom < 12.0:
            safe_denom = 12.0
        raw_s = hp_total / safe_denom
        safety = 0.22 if raw_s < 0.22 else (1.05 if raw_s > 1.05 else raw_s)

        raw_h = (16.5 - turn) / 9.0
        horizon = 0.22 if raw_h < 0.22 else (1.40 if raw_h > 1.40 else raw_h)

        owns_gauntlet = any(tid in self._gauntlet_ids for tid in obs.trinkets)
        enemy_giant = particles.has_giant_unit if particles is not None else False
        enemy_sum = particles.has_summoners if particles is not None else False
        col_cnt = (
            max(particles.colossus_count, particles.lobby_colossus_count)
            if (particles is not None and not owns_gauntlet)
            else (particles.colossus_count if particles is not None else 0)
        )
        enemy_ds = particles.has_divine_shields if particles is not None else False

        tribe_counts: Dict[str, int] = {}
        board_copy_counts: Dict[int, int] = {}
        summoners = 0
        combat_obs = 0
        eot_engines = 0
        play_engines = 0
        spell_engines = 0
        gold_engines = 0
        mag_engines = 0
        bc_engines = 0
        battlecries = 0
        has_brann = 0
        has_drakkari = 0
        has_titus = 0
        has_murkeye = 0
        has_poet = 0
        ds_count = 0
        boom_count = 0

        b_id = self._brann_id
        d_id = self._drakkari_id
        t_id = self._titus_id
        m_id = self._murkeye_id
        p_id = self._poet_id
        boom_id = self._boom_id
        profiles = self.meta.profiles

        board = obs.board
        for u in board:
            for tr in u.tribes:
                tribe_counts[tr] = tribe_counts.get(tr, 0) + 1
            cid = u.card_id
            if not u.is_golden and not u.is_spell:
                board_copy_counts[cid] = board_copy_counts.get(cid, 0) + 1
            if u.divine_shield:
                ds_count += 1
            if cid == boom_id:
                boom_count += 1

            mult = 2 if u.is_golden else 1
            if cid == b_id:
                has_brann += mult
            elif cid == d_id:
                has_drakkari += mult
            elif cid == t_id:
                has_titus += mult
            elif cid == m_id:
                has_murkeye += mult
            elif cid == p_id:
                has_poet += mult

            prof = profiles.get(cid)
            if prof is not None:
                if prof.is_combat_summoner or u.reborn:
                    summoners += 1
                if prof.has_avenge or prof.is_combat_observer:
                    combat_obs += mult
                if prof.has_end_of_turn:
                    eot_engines += mult
                if prof.triggers_on_play:
                    play_engines += mult
                if prof.triggers_on_spell:
                    spell_engines += mult
                if prof.triggers_on_gold_or_buy:
                    gold_engines += mult
                if prof.triggers_on_magnetize:
                    mag_engines += mult
                if prof.triggers_on_battlecry:
                    bc_engines += mult
                if prof.has_battlecry:
                    battlecries += 1

        ehp_clamped = hp_total if hp_total < 60.0 else 60.0
        score = 9.5 * math.log1p(ehp_clamped / 8.0)
        next_danger = float(dmg_cap) if dmg_cap is not None else 18.0
        if hp_total <= next_danger + 2.0:
            ehp_def = (next_danger + 3.0 - hp_total) / max(6.0, next_danger)
            score -= 8.0 * (ehp_def ** 1.35)

        weakest_board_score = 1e9
        has_last_standing = 0
        obs_trinkets = obs.trinkets
        for u in board:
            if u.card_id == self._last_standing_id:
                has_last_standing += 2 if u.is_golden else 1
            uv = self.unit_value(
                u,
                turn,
                tribe_counts,
                enemy_giant,
                enemy_sum,
                on_board=True,
                colossus_count=col_cnt,
                enemy_has_ds=enemy_ds,
                board_units=board,
                obs_trinkets=obs_trinkets,
                lobby_tribes=lobby_tribes,
            )
            score += uv
            if uv < weakest_board_score:
                weakest_board_score = uv
        if not board:
            weakest_board_score = 0.0
        if has_last_standing > 0:
            distinct_tr = min(7, len(tribe_counts) + 2 * tribe_counts.get("all", 0))
            score += 4.5 * has_last_standing * distinct_tr * safety

        if boom_count > 0 and ds_count >= 2:
            score -= 7.5 * ds_count

        n_board = len(board)
        slot_w = 3.0 if turn <= 6 else 2.5
        score += slot_w * (n_board if n_board < 7 else 7)
        if end_of_turn and turn >= 7 and n_board < 7:
            score -= 11.5 * (7 - n_board)
        elif not end_of_turn and 6 <= turn <= 10:
            min_mid_slots = 6 if turn >= 7 else 5
            if n_board < min_mid_slots:
                score -= 4.5 * (min_mid_slots - n_board)

        if tribe_counts:
            dom_cnt = max(tribe_counts.values())
            if dom_cnt >= 3:
                score += 1.38 * ((dom_cnt - 2) ** 1.45) * safety

        if summoners > 0 and (combat_obs > 0 or has_titus > 0):
            score += 2.8 * min(4, summoners) * min(5, combat_obs + 2 * has_titus) * safety
        if has_drakkari > 0 and eot_engines > 0:
            score += 4.5 * has_drakkari * min(5, eot_engines) * horizon * safety
        elif eot_engines >= 2:
            score += 1.9 * (eot_engines - 1) * horizon * safety
        if (bc_engines > 0 or has_brann > 0 or has_murkeye > 0) and battlecries > 0:
            score += (
                2.9
                * (bc_engines + 2 * has_brann + 3 * has_murkeye)
                * min(4, battlecries)
                * horizon
                * safety
            )
        has_dragon_combat_buf = any(
            u.card_id in self._dragon_combat_buffer_ids for u in board
        ) or any(tid in self._dragon_combat_trinket_ids for tid in obs_trinkets)
        if has_poet > 0 and has_dragon_combat_buf and (tribe_counts.get("dragon", 0) + tribe_counts.get("all", 0)) >= 2:
            drag_cnt = tribe_counts.get("dragon", 0) + tribe_counts.get("all", 0)
            score += 4.2 * has_poet * min(4, drag_cnt - 1) * horizon * safety
        if mag_engines > 0 and tribe_counts.get("mech", 0) >= 2:
            score += 3.0 * mag_engines * min(5, tribe_counts.get("mech", 0)) * horizon * safety
        if play_engines > 0:
            score += 2.0 * play_engines * horizon * safety
        if gold_engines > 0:
            score += 2.2 * gold_engines * horizon * safety
        if spell_engines > 0:
            score += 2.0 * spell_engines * horizon * safety

        # 2. Tavern Tier curve + Smooth Bargain/Anti-Stall Upgrade Discipline
        tier = obs.tavern_tier
        if tier < 1:
            tier = 1
        elif tier > 7:
            tier = 7
        owns_candle = self._candle_id > 0 and any(tid == self._candle_id for tid in obs_trinkets)
        raw_th = (19.5 - turn) / 8.5
        tier_horizon = 0.62 if raw_th < 0.62 else (1.25 if raw_th > 1.25 else raw_th)
        if owns_candle and tier > 4:
            base_tv = _SOV_TIER_CURVE[4] + 0.30 * (_SOV_TIER_CURVE[tier] - _SOV_TIER_CURVE[4])
        else:
            base_tv = _SOV_TIER_CURVE[tier]
        tier_val = base_tv
        if alive_units <= 2 and turn <= 6 and (hp_total < 30.0 or n_board < min(6, turn)):
            tier_val *= 0.85
        score += tier_val * tier_horizon * safety

        if turn >= 4 and tier == 1:
            score -= 6.5 * (turn - 2)

        is_duos = hasattr(obs, "team_health")
        cheap_upg = 7 if is_duos else 6
        if is_duos and getattr(obs, "teammate_tavern_tier", 0) > tier and obs.upgrade_cost >= 8:
            score -= 3.0

        # Smooth, cliff-free upgrade discipline (never rewards losing HP to cross a threshold!)
        if not owns_candle and n_board >= 6:
            upg_cost = int(obs.upgrade_cost)
            upg_safety = max(0.0, min(1.0, (hp_total - 15.5) / 11.5))
            bargain_safety = max(0.0, min(1.0, (hp_total - 10.5) / 7.5))
            if upg_cost == 0 and tier < 6 and turn >= 5 and hp_total >= 4.0:
                score -= 11.5
            if turn >= 7 and tier < 3 and upg_cost <= 3:
                score -= 10.5 * (3 - tier) * bargain_safety
            elif turn >= 8 and tier < 4 and upg_cost <= 2:
                score -= 13.5 * (4 - tier) * bargain_safety
            elif turn >= 8 and tier < 4 and upg_cost <= cheap_upg:
                score -= 9.0 * (4 - tier) * upg_safety
            elif turn >= 9 and tier < 4:
                score -= (10.5 if upg_cost <= 4 else 9.0) * (4 - tier) * upg_safety
            elif turn >= 10 and tier < 5 and upg_cost <= 2:
                score -= 12.5 * bargain_safety
            elif turn >= 10 and tier < 5 and upg_cost <= cheap_upg + 1:
                score -= 9.0 * upg_safety
            elif turn >= 11 and tier < 5:
                score -= 10.5 * upg_safety
            elif turn >= 12 and tier < 6 and upg_cost <= 2:
                score -= 10.5 * bargain_safety
            elif turn >= 12 and tier < 6 and upg_cost <= cheap_upg + 1:
                score -= 8.0 * max(0.0, min(1.0, (hp_total - 16.0) / 11.0))
        if col_cnt >= 3 and tier < 5 and hp_total >= 15.0 and n_board >= 6 and not owns_candle:
            score -= 14.0 * (5 - tier) * max(0.35, min(1.0, (hp_total - 12.0) / 12.0))

        if owns_gauntlet:
            if has_drakkari > 0:
                score += 55.0 * has_drakkari * safety
            for u in board:
                u_prof = profiles.get(u.card_id)
                if (
                    u.divine_shield
                    or u.venomous
                    or u.reborn
                    or u.windfury
                    or (u_prof is not None and u_prof.has_cleave)
                ):
                    score += 10.5 * safety

        if tier < 6:
            nominal_upg = _BASE_UPGRADE_COST[tier]
            discount = nominal_upg - int(obs.upgrade_cost)
            if discount > 0:
                capped_disc = discount if discount < 4 else 4
                score += 0.50 * capped_disc * horizon * safety

        # 3. Hand & Handbuff valuation
        hand_synergy = self.has_hand_buff_synergy(obs)
        all_copy_counts = dict(board_copy_counts)
        playable_hand_minions = 0
        for h_idx, c in enumerate(obs.hand):
            cid = c.card_id
            prof = profiles.get(cid)
            is_intrinsic_gold = prof.is_intrinsic_golden if prof is not None else False
            if not c.is_golden and not c.is_spell:
                all_copy_counts[cid] = all_copy_counts.get(cid, 0) + 1

            if c.is_golden and not c.is_spell and not is_intrinsic_gold:
                if end_of_turn and not c.unplayable:
                    if hand_synergy and n_board >= 7 and (c.attack + c.health) >= 35:
                        score += 0.45 * _concave_stat_value(c.attack, c.health)
                    else:
                        score -= 6.0
                else:
                    score += 8.5 * safety
            elif c.is_spell:
                is_temp = prof.is_temporary if prof is not None else False
                if end_of_turn:
                    score += -2.0 if is_temp else -0.35
                elif cid == self._upper_hand_id:
                    uh_val = 36.0 if col_cnt >= 2 else (11.5 if enemy_giant else 5.0)
                    score += uh_val
                else:
                    score += 3.6 if c.spell_cost == 0 else 1.8
            else:
                if c.unplayable:
                    score += (2.2 + 0.6 * c.tavern_tier + 0.25 * _concave_stat_value(c.attack, c.health)) * safety
                elif end_of_turn:
                    playable_hand_minions += 1
                    if n_board < 7:
                        score -= 10.5
                    elif hand_synergy and h_idx <= 1:
                        score += (1.5 + 0.55 * _concave_stat_value(c.attack, c.health)) * safety
                    else:
                        cand_val = self.unit_value(
                            c,
                            turn,
                            tribe_counts,
                            enemy_giant,
                            enemy_sum,
                            on_board=False,
                            colossus_count=col_cnt,
                            enemy_has_ds=enemy_ds,
                            board_units=board,
                            obs_trinkets=obs_trinkets,
                            lobby_tribes=lobby_tribes,
                        )
                        if cand_val > weakest_board_score + 4.5:
                            score -= 4.5
                        elif playable_hand_minions == 1 and len(obs.hand) <= 6:
                            score += 0.25 * safety
                        else:
                            score -= 3.2
                    if len(obs.hand) >= 8 and not hand_synergy:
                        score -= 2.5 * (len(obs.hand) - 7)
                else:
                    if n_board < 7:
                        cand_val = self.unit_value(
                            c,
                            turn,
                            tribe_counts,
                            enemy_giant,
                            enemy_sum,
                            on_board=False,
                            colossus_count=col_cnt,
                            enemy_has_ds=enemy_ds,
                            board_units=board,
                            obs_trinkets=obs_trinkets,
                            lobby_tribes=lobby_tribes,
                        )
                        score += max(2.2 + 0.50 * c.tavern_tier, 0.85 * cand_val) * safety
                    elif hand_synergy and h_idx == 0:
                        score += (1.8 + 0.45 * _concave_stat_value(c.attack, c.health)) * safety
                    else:
                        cand_val = self.unit_value(
                            c,
                            turn,
                            tribe_counts,
                            enemy_giant,
                            enemy_sum,
                            on_board=False,
                            colossus_count=col_cnt,
                            enemy_has_ds=enemy_ds,
                            board_units=board,
                            obs_trinkets=obs_trinkets,
                            lobby_tribes=lobby_tribes,
                        )
                        swap_gain = cand_val - weakest_board_score + 1.80
                        if swap_gain < 0.0:
                            swap_gain = 0.0
                        bc_bonus = 2.6 if (prof is not None and prof.has_battlecry) else 0.0
                        c_eng = self.effective_engine_bonus(
                            cid,
                            turn,
                            tribe_counts,
                            on_board=False,
                            board_units=board,
                            is_golden=c.is_golden,
                            obs_trinkets=obs_trinkets,
                            lobby_tribes=lobby_tribes,
                        )
                        base_hold = 0.65 + 0.30 * c.tavern_tier + 0.35 * c_eng
                        score += max(base_hold, 0.88 * swap_gain + bc_bonus) * safety

        # 4. Pair potential
        pairs_counted = 0
        tmpls = self.meta.templates_by_id
        for cid, cnt in all_copy_counts.items():
            if cnt == 2 and pairs_counted < 2:
                tmpl = tmpls.get(cid)
                t_val = tmpl.tavern_tier if tmpl else 1
                eng_b = self.effective_engine_bonus(
                    cid, turn, tribe_counts, on_board=True, lobby_tribes=lobby_tribes
                )
                if turn <= 6 or t_val >= max(2, tier - 1) or eng_b >= 8.0 or board_copy_counts.get(cid, 0) == 2:
                    pairs_counted += 1
                    score += (1.5 + 0.40 * min(6, tier + 1) + 0.30 * t_val + 0.15 * eng_b) * safety

        # 5. Frozen / carried-over shop valuation (ONLY score deterministic frozen shops — never random unfrozen t+1 shop draws!)
        if obs.is_frozen:
            score += self._freeze_shop_value(
                obs, all_copy_counts, tribe_counts, lobby_tribes=lobby_tribes
            ) * safety
        elif in_dist_sot and sot_shop_is_real:
            score += 0.42 * self._sot_shop_opportunity_value(
                obs, all_copy_counts, tribe_counts, lobby_tribes
            ) * safety

        # 6. Persistent Player Counters, Deity, Card Counters & Trinkets
        next_gold_surv = max(0.15, min(1.0, (hp_total - 8.0) / 8.0))
        score += 1.85 * obs.bonus_gold_next_turn * safety * next_gold_surv
        pc = obs.player_counters
        n_pc = len(pc)

        if 0 <= self._c_max_gold < n_pc:
            mg = pc[self._c_max_gold]
            if mg:
                score += 3.5 * mg * horizon * safety

        qb_mult = 1.0 + 0.42 * tribe_counts.get("quilboar", 0)
        if 0 <= self._c_gem_atk < n_pc and 0 <= self._c_gem_hp < n_pc:
            gems = pc[self._c_gem_atk] + pc[self._c_gem_hp]
            if gems:
                score += 1.15 * gems * qb_mult * safety

        if 0 <= self._c_sp_atk < n_pc and 0 <= self._c_sp_hp < n_pc:
            sps = pc[self._c_sp_atk] + pc[self._c_sp_hp]
            if sps:
                score += (0.95 + 0.40 * spell_engines) * sps * safety

        if 0 <= self._c_el_atk < n_pc and 0 <= self._c_el_hp < n_pc:
            els = pc[self._c_el_atk] + pc[self._c_el_hp]
            if els:
                el_mult = 0.75 + 0.40 * tribe_counts.get("elemental", 0)
                score += 0.85 * els * el_mult * safety

        if 0 <= self._c_all_atk < n_pc and 0 <= self._c_all_hp < n_pc:
            alls = pc[self._c_all_atk] + pc[self._c_all_hp]
            if alls:
                score += 1.05 * alls * safety

        if 0 <= self._c_ud_atk < n_pc and 0 <= self._c_ud_hp < n_pc:
            uds = pc[self._c_ud_atk] + pc[self._c_ud_hp]
            if uds:
                ud_mult = 0.65 + 0.50 * tribe_counts.get("undead", 0)
                score += 0.95 * uds * ud_mult * safety

        for tr_name, idx_a, idx_h in self._tribe_counter_idx:
            if idx_a < n_pc and idx_h < n_pc:
                tb = pc[idx_a] + pc[idx_h]
                if tb > 0:
                    score += 0.60 * tb * (0.6 + 0.40 * tribe_counts.get(tr_name, 0)) * safety

        if 0 <= self._c_ref_gems < n_pc:
            rg = pc[self._c_ref_gems]
            if rg:
                score += 1.25 * rg * qb_mult * safety
        if 0 <= self._c_ref_buffs < n_pc:
            rb = pc[self._c_ref_buffs]
            if rb:
                score += 0.80 * rb * safety

        if obs.deity != "none" and tribe_counts.get("aberration", 0) >= 1:
            ab_mult = min(1.5, 0.35 * tribe_counts.get("aberration", 0))
            score += 0.52 * _concave_stat_value(obs.deity_attack, obs.deity_health) * ab_mult

        for _, v1, v2 in obs.card_counters:
            score += 0.60 * ((v1 if v1 > 0 else 0) + (v2 if v2 > 0 else 0)) * safety

        for eff_id, eff_stacks, _ in obs.effects:
            if eff_id == self._upper_hand_id:
                uh_eff = 42.0 if col_cnt >= 2 else (13.5 if enemy_giant else 6.0)
                score += uh_eff * eff_stacks
            else:
                score += 2.8 * eff_stacks

        tp_map = self._trinket_prior_by_id
        t_counters = obs.trinket_counters
        for ti, tid in enumerate(obs.trinkets):
            t_prior = tp_map.get(tid, 3.5)
            score += (2.5 + 0.35 * t_prior * horizon) * safety
            if tid == self._candle_id and ti < len(t_counters):
                candle_left = 2 - t_counters[ti]
                if candle_left > 0:
                    if not end_of_turn and obs.gold >= 4:
                        score += 4.2 * candle_left
                    elif end_of_turn and obs.gold >= 4:
                        score -= 5.5 * candle_left

        dd_left = getattr(obs, "dark_discoveries_left", 0)
        if dd_left > 0:
            dd_unit = 2.2 if (dmg_cap is None or alive_units <= 3 or hp_total <= 22.0) else 4.5
            score += dd_unit * dd_left * horizon * safety

        if not end_of_turn:
            gold_unit_val = 1.78 + 0.08 * max(0, tier - 2) + 0.12 * min(3, gold_engines)
            ref_unit_val = 1.15 + 0.10 * max(0, tier - 3)
            score += (
                gold_unit_val * obs.gold
                + ref_unit_val * obs.free_refreshes
                + 0.80 * max(0, obs.next_minion_discount)
            )
            if obs.discover_pending:
                score += 7.5 + 1.2 * tier

        return score

    def _freeze_shop_value(
        self,
        obs: seaglass.TavernObservation,
        all_copies: Dict[int, int],
        tribe_counts: Optional[Dict[str, int]] = None,
        lobby_tribes: Optional[Sequence[str]] = None,
    ) -> float:
        # Allow freezing when `< minion_cost` OR when `gold <= 3` on a full 7-board where buying now hurts tempo!
        if obs.gold > max(3, obs.minion_cost):
            return -2.4
        if obs.gold >= obs.minion_cost and len(obs.board) < 7:
            return -2.2

        tier = obs.tavern_tier
        turn = obs.turn
        if tribe_counts is None:
            tribe_counts = {}
            for u in obs.board:
                for tr in u.tribes:
                    tribe_counts[tr] = tribe_counts.get(tr, 0) + 1

        owns_candle = self._candle_id > 0 and any(tid == self._candle_id for tid in obs.trinkets)
        target_vals: List[float] = []
        for s in obs.shop:
            if s.is_spell:
                if s.spell_cost >= 2 and obs.gold < s.spell_cost and turn <= 8:
                    target_vals.append(0.6)
                continue
            eng_b = self.effective_engine_bonus(
                s.card_id,
                turn,
                tribe_counts,
                on_board=False,
                board_units=obs.board,
                obs_trinkets=obs.trinkets,
                lobby_tribes=lobby_tribes,
            )
            if not s.is_golden and all_copies.get(s.card_id, 0) >= 2:
                invested_stats = sum(
                    u.attack + u.health for u in obs.board if u.card_id == s.card_id
                )
                if s.tavern_tier >= max(1, tier - 2) or turn <= 7 or invested_stats >= 22 or eng_b >= 8.0:
                    target_vals.append(2.8)
            else:
                if eng_b >= 12.0 and s.tavern_tier >= max(4, tier):
                    target_vals.append(2.6)
                elif not owns_candle and eng_b >= 9.0 and s.tavern_tier >= max(4, tier):
                    target_vals.append(1.9)
                elif not owns_candle and s.tavern_tier >= max(4, tier + 1) and eng_b > 0.0:
                    target_vals.append(1.3)

        if not target_vals:
            return -2.0
        target_vals.sort(reverse=True)
        best_fz = target_vals[0]
        if len(target_vals) >= 2:
            best_fz += 0.45 * target_vals[1]
        return best_fz

    def freeze_shop_value(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> float:
        """The number `should_freeze_current_shop` compares to `FREEZE_THRESHOLD`."""
        all_copies: Dict[int, int] = {}
        tribe_counts: Dict[str, int] = {}
        for u in obs.board:
            for tr in u.tribes:
                tribe_counts[tr] = tribe_counts.get(tr, 0) + 1
            if not u.is_golden and not u.is_spell:
                all_copies[u.card_id] = all_copies.get(u.card_id, 0) + 1
        for c in obs.hand:
            if not c.is_golden and not c.is_spell:
                all_copies[c.card_id] = all_copies.get(c.card_id, 0) + 1
        lobby_tribes = ctx.tribes if ctx is not None else None
        return self._freeze_shop_value(obs, all_copies, tribe_counts, lobby_tribes=lobby_tribes)

    FREEZE_THRESHOLD = 1.5

    def should_freeze_current_shop(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> bool:
        """Fast helper for Search/Zero-Waste guard: True if current shop has a high-EV freeze target."""
        return self.freeze_shop_value(obs, ctx=ctx) >= self.FREEZE_THRESHOLD

    def evaluate_fast_batch(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = False,
        particles: Optional[SovereignOpponentParticles] = None,
        in_dist_sot: bool = False,
        sot_shop_is_real: bool = False,
        parts: Optional[List[Dict[str, float]]] = None,
    ) -> List[float]:
        """`parts`: optional out-list; one dict per obs with `prior`, `net` (and `net_raw` when used)."""
        if not obs_list:
            return []
        prior_scores = [
            self.macro_and_economy_value(
                o,
                ctx=ctx,
                end_of_turn=end_of_turn,
                particles=particles,
                in_dist_sot=in_dist_sot,
                sot_shop_is_real=sot_shop_is_real,
            )
            for o in obs_list
        ]
        # Pure 8-Player FFA `SovereignNumpyPredictor` runs in ~0.045ms and is valid for ALL `alive_units`!
        if (
            not (in_dist_sot or end_of_turn)
            or not self.has_neural_model
            or self.predictor is None
            or (ctx is not None and getattr(ctx, "duos", False))
        ):
            if parts is not None:
                parts.extend({"prior": float(p), "net": 0.0} for p in prior_scores)
            return prior_scores

        net_scores = self.predictor.predict_batch(obs_list, ctx=ctx, end_of_turn=end_of_turn)
        w = self.neural_weight
        if parts is not None:
            parts.extend(
                {"prior": float(p), "net": float(w * n), "net_raw": float(n)}
                for p, n in zip(prior_scores, net_scores)
            )
        return [p_val + w * n_val for p_val, n_val in zip(prior_scores, net_scores)]

    def evaluate_fast(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = False,
        particles: Optional[SovereignOpponentParticles] = None,
        in_dist_sot: bool = False,
    ) -> float:
        return self.evaluate_fast_batch(
            [obs],
            ctx=ctx,
            end_of_turn=end_of_turn,
            particles=particles,
            in_dist_sot=in_dist_sot,
        )[0]

    def _pre_end_turn_discipline(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> float:
        adj = 0.0
        hp_total = max(1.0, self.effective_hp(obs))
        safety = min(1.05, max(0.22, hp_total / 20.0))
        hand_syn = self.has_hand_buff_synergy(obs)
        lobby_tribes = ctx.tribes if ctx is not None else None

        all_copies: Dict[int, int] = {}
        tribe_counts: Dict[str, int] = {}
        for u in obs.board:
            for tr in u.tribes:
                tribe_counts[tr] = tribe_counts.get(tr, 0) + 1
            if not u.is_golden and not u.is_spell:
                all_copies[u.card_id] = all_copies.get(u.card_id, 0) + 1
        for c in obs.hand:
            if not c.is_golden and not c.is_spell:
                all_copies[c.card_id] = all_copies.get(c.card_id, 0) + 1

        playable_minions = 0
        for h_idx, c in enumerate(obs.hand):
            c_prof = self.meta.profiles.get(c.card_id)
            is_intr_g = c_prof.is_intrinsic_golden if c_prof is not None else False
            if c.is_golden and not c.is_spell and not c.unplayable and not is_intr_g:
                if not (hand_syn and len(obs.board) >= 7 and (c.attack + c.health) >= 35):
                    adj -= 10.5
            elif c.is_spell:
                is_temp = c_prof.is_temporary if c_prof is not None else False
                if is_temp:
                    adj -= 2.8
            elif not c.unplayable:
                playable_minions += 1
                if len(obs.board) < 7:
                    adj -= 14.0
                elif hand_syn and h_idx <= 1:
                    adj += 0.0
                elif playable_minions == 1:
                    adj -= 1.0
                else:
                    adj -= 2.8

        if obs.turn >= 7 and len(obs.board) < 7:
            adj -= 12.0 * (7 - len(obs.board))

        if len(obs.hand) >= 8 and not hand_syn:
            adj -= 5.5 * (len(obs.hand) - 7)

        if obs.is_frozen:
            adj += (
                self._freeze_shop_value(
                    obs, all_copies, tribe_counts, lobby_tribes=lobby_tribes
                )
                * safety
            )
        return adj

    def evaluate_turn_end(
        self,
        tavern: seaglass.TavernGame,
        particles: SovereignOpponentParticles,
        ctx: Optional[seaglass.LobbyContext] = None,
        combats_per_particle: int = 14,
        board_override: Optional[List[seaglass.Unit]] = None,
        pre_positioned: bool = False,
        parts: Optional[Dict[str, float]] = None,
    ) -> float:
        """
        `parts`: optional out-dict filled with the score breakdown (see sovereign_trace.PARTS_KEYS).

        Positioned 1-Turn Physics + CVaR Survival + Pure-8p-FFA Macro Evaluation:
          1. Reposition `combat_game` in `<0.05ms` via `fast_reposition_game` BEFORE `end_turn()`
             so `Young Murk-Eye`, `Persistent Poet`, `Stitched Salvager`, Cleaves & `Titus` fire properly!
          2. Run Monte Carlo `equity_against` on opponent particles + CVaR highroll check.
          3. Step `resolve_combat_against` + `start_turn()` -> evaluate Turn `t + 1` state.
        """
        obs_pre = tavern.observe()
        hp_now = self.effective_hp(obs_pre)
        alive_units = ctx.alive_units if ctx is not None else 8
        damage_cap = ctx.damage_cap if ctx is not None else None
        is_ghost = particles.is_ghost or (ctx.next_is_ghost if ctx is not None else False)
        lobby_hps = (
            [o.health + o.armor for o in ctx.opponents if o.alive]
            if ctx is not None
            else [30] * 7
        )

        if self.use_physics_rollout and board_override is None:
            combat_game = tavern.copy()
            if not pre_positioned:
                self.positioner.fast_reposition_game(combat_game, obs_pre, particles)
            combat_game.reseed((obs_pre.turn * 65537 + 17) | 1)
            if combat_game.is_legal(seaglass.TavernAction.end_turn()):
                combat_game.end_turn()
        else:
            combat_game = tavern

        icm_sum = 0.0
        offense_bonus_sum = 0.0
        lethal_prob_sum = 0.0
        n_boards = max(1, len(particles.boards))
        is_endgame = alive_units <= (2 if (ctx is not None and ctx.duos) else 4)
        ko_mult = 1.65 if is_endgame else 1.05

        u_win_or_draw = self.cvar_survival_utility(
            hp_now, alive_units, lobby_hps, damage_cap
        )

        for idx, opp_board in enumerate(particles.boards):
            dist = combat_game.equity_against(
                opp_board,
                opponent_tier=particles.tier,
                n=combats_per_particle,
                base_seed=obs_pre.turn * 1009 + idx * 37 + 1,
                board=board_override,
            )
            if dist.b_win_rate > 1e-6:
                worst_loss = float(max(0, -dist.damage_min))
                if damage_cap is not None:
                    worst_loss = min(worst_loss, float(damage_cap))
                avg_loss = min(
                    worst_loss,
                    max(float(particles.tier + 1), 0.65 * worst_loss),
                )
                if damage_cap is not None:
                    avg_loss = min(avg_loss, float(damage_cap))
            else:
                worst_loss = 0.0
                avg_loss = 0.0

            if hp_now - avg_loss <= 0.0:
                lethal_prob_sum += dist.b_win_rate
            elif hp_now - worst_loss <= 0.0:
                lethal_prob_sum += 0.45 * dist.b_win_rate

            u_avg_loss = self.cvar_survival_utility(
                hp_now - avg_loss, alive_units, lobby_hps, damage_cap
            )
            u_worst_loss = self.cvar_survival_utility(
                hp_now - worst_loss, alive_units, lobby_hps, damage_cap
            )
            loss_utility = 0.68 * u_avg_loss + 0.32 * u_worst_loss

            expected_icm = (
                (dist.a_win_rate + dist.draw_rate) * u_win_or_draw
                + dist.b_win_rate * loss_utility
            )
            icm_sum += expected_icm

            if not is_ghost and dist.a_win_rate > 0.0:
                dealt = max(0.0, dist.damage_mean)
                if damage_cap is not None:
                    dealt = min(dealt, float(damage_cap))
                offense_bonus_sum += ko_mult * (0.31 * dealt + 3.0 * dist.a_win_rate)

        cvar_penalty = 0.0
        if (
            not is_ghost
            and particles.highroll_board
            and (damage_cap is None or hp_now <= float(damage_cap) + 6.0)
        ):
            hr_dist = combat_game.equity_against(
                particles.highroll_board,
                opponent_tier=min(6, particles.tier + 1),
                n=max(6, combats_per_particle // 2),
                base_seed=obs_pre.turn * 3571 + 99,
                board=board_override,
            )
            if hr_dist.b_win_rate > 1e-6:
                hr_worst = float(max(0, -hr_dist.damage_min))
                if damage_cap is not None:
                    hr_worst = min(hr_worst, float(damage_cap))
                if hp_now - hr_worst <= 0.0:
                    cvar_penalty = 38.0 * hr_dist.b_win_rate

        combat_icm_score = (icm_sum + offense_bonus_sum) / n_boards - cvar_penalty
        boss_bonus_term = 0.0

        if (
            obs_pre.turn >= 10
            and hp_now >= 14.0
            and particles.lobby_colossus_count >= 2
            and particles.lobby_boss_board
            and particles.lobby_boss_player >= 0
            and particles.lobby_boss_player != particles.opponent_player
            and not any(tid in self._gauntlet_ids for tid in obs_pre.trinkets)
        ):
            boss_dist = combat_game.equity_against(
                particles.lobby_boss_board,
                opponent_tier=particles.lobby_boss_tier,
                n=6,
                base_seed=obs_pre.turn * 4999 + 31,
                board=board_override,
            )
            boss_bonus = 22.0 * (boss_dist.a_win_rate + 0.5 * boss_dist.draw_rate) + 0.20 * max(
                0.0, boss_dist.damage_mean
            )
            boss_bonus_term = 0.30 * boss_bonus
            combat_icm_score += boss_bonus_term

        survival_prob = max(0.0, min(1.0, 1.0 - (lethal_prob_sum / n_boards)))

        if self.use_physics_rollout and board_override is None and particles.boards:
            next_obs_samples: List[seaglass.TavernObservation] = []
            n_phys = min(2, len(particles.boards))
            for p_idx in range(n_phys):
                g_next = combat_game.copy()
                c_seed = (obs_pre.turn * 131071 + p_idx * 8191 + 3) | 1
                g_next.reseed(c_seed)
                g_next.resolve_combat_against(
                    particles.boards[p_idx],
                    opponent_tier=particles.tier,
                    seed=c_seed,
                )
                # Decouple Start-of-Turn RNG from combat RNG shifts across candidates!
                g_next.reseed((obs_pre.turn * 524287 + p_idx * 16381 + 11) | 1)
                g_next.start_turn()
                next_obs_samples.append(g_next.observe())

            next_parts: Optional[List[Dict[str, float]]] = [] if parts is not None else None
            next_scores = self.evaluate_fast_batch(
                next_obs_samples,
                ctx=ctx,
                end_of_turn=False,
                particles=particles,
                in_dist_sot=True,
                sot_shop_is_real=obs_pre.is_frozen,
                parts=next_parts,
            )
            avg_next_macro = sum(next_scores) / len(next_scores)
            pre_discipline = self._pre_end_turn_discipline(obs_pre, ctx=ctx)
            continuation_weight = 0.30 + 0.70 * survival_prob
            macro_score = continuation_weight * avg_next_macro + pre_discipline
            if parts is not None and next_parts:
                parts["continuation_weight"] = float(continuation_weight)
                parts["next_prior"] = sum(x["prior"] for x in next_parts) / len(next_parts)
                parts["next_net"] = sum(x["net"] for x in next_parts) / len(next_parts)
                parts["discipline"] = float(pre_discipline)
        else:
            fast_parts: Optional[List[Dict[str, float]]] = [] if parts is not None else None
            macro_score = self.evaluate_fast_batch(
                [obs_pre], ctx=ctx, end_of_turn=True, particles=particles, parts=fast_parts
            )[0]
            if parts is not None and fast_parts:
                parts["continuation_weight"] = 0.0
                parts["next_prior"] = float(fast_parts[0]["prior"])
                parts["next_net"] = float(fast_parts[0]["net"])
                parts["discipline"] = 0.0

        total = combat_icm_score + macro_score
        if parts is not None:
            parts.update(
                combat_icm=float(icm_sum / n_boards),
                offense=float(offense_bonus_sum / n_boards),
                cvar_penalty=float(cvar_penalty),
                boss_bonus=float(boss_bonus_term),
                combat=float(combat_icm_score),
                survival_prob=float(survival_prob),
                macro=float(macro_score),
                total=float(total),
            )
        return total

    def choose_best_hero(
        self,
        obs: seaglass.TavernObservation,
        legal: Sequence[seaglass.TavernAction],
        ctx: Optional[seaglass.LobbyContext] = None,
        base_game: Optional[seaglass.TavernGame] = None,
    ) -> seaglass.TavernAction:
        """
        Select offered Hero using:
          1. 5-Tribe Lobby Conditioning (`SovereignKnowledge.hero_lobby_score(hid, ctx.tribes)`),
          2. Empirical 8-ApexBot self-play placement deltas + starting Armor/EHP,
          3. Batched pure-8p-FFA `SovereignNumpyPredictor` conditioned on `ctx.tribes`!
        """
        lobby_tribes = list(ctx.tribes) if ctx is not None else []
        candidates: List[Tuple[seaglass.TavernAction, float, Optional[seaglass.TavernObservation]]] = []

        for act in legal:
            if act.kind != "choose_hero":
                continue
            opt_idx, _ = act.args
            if opt_idx is None or opt_idx >= len(obs.hero_options):
                continue
            hid = obs.hero_options[opt_idx]
            score = self.sov_kb.hero_lobby_score(hid, lobby_tribes)

            prof = self.meta.profiles.get(hid)
            if prof is not None:
                score += 0.65 * prof.engine_weight
                if prof.is_economy_generator:
                    score += 2.0

            sim_obs: Optional[seaglass.TavernObservation] = None
            if base_game is not None:
                sim = base_game.copy()
                if sim.is_legal(act):
                    sim.step(act)
                    sim_obs = sim.observe()
                    ehp = self.effective_hp(sim_obs)
                    score += 0.34 * (ehp - 30.0)

            candidates.append((act, score, sim_obs))

        if not candidates:
            return legal[0]

        if (
            self.has_neural_model
            and self.predictor is not None
            and all(c[2] is not None for c in candidates)
            and (ctx is None or not getattr(ctx, "duos", False))
        ):
            sim_obs_list = [c[2] for c in candidates if c[2] is not None]
            net_vals = self.predictor.predict_batch(
                sim_obs_list, ctx=ctx, end_of_turn=False
            )
            best_idx = max(
                range(len(candidates)),
                key=lambda i: candidates[i][1] + 0.25 * net_vals[i],
            )
            return candidates[best_idx][0]

        best_act = max(candidates, key=lambda c: c[1])[0]
        return best_act
