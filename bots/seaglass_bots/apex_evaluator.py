"""Apex Evaluator: Multi-Turn Rust Physics, CVaR Zero-Death Survival Shield & Set-Transformer Fusion."""

from __future__ import annotations

import math
from typing import Dict, FrozenSet, List, Optional, Sequence, Tuple

import seaglass

from .apex_belief import ApexOpponentParticles
from .apex_knowledge import (
    HERO_EQUITY_PRIORS,
    TRINKET_EQUITY_PRIORS,
    ApexKnowledge,
    get_apex_knowledge,
)
from .apex_model import ApexValuePredictor, get_apex_predictor

# Calibrated multi-turn Tavern Tier curve (dominant Turn 1-6 board tempo + crisp Tier 4->5->6 scaling).
_APEX_TIER_CURVE = (0.0, 0.0, 9.8, 18.0, 29.0, 46.5, 64.0, 77.5)

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
    """Linear stat value up to 80/80, smoothly concave above 80 (zero inner-closure allocations)."""
    a = atk if atk > 0.0 else 0.0
    h = hp if hp > 0.0 else 0.0
    ca = a if a <= 80.0 else (80.0 + 22.0 * math.sqrt(a - 80.0))
    ch = h if h <= 80.0 else (80.0 + 22.0 * math.sqrt(h - 80.0))
    return 0.49 * ca + 0.47 * ch


def _target_tier_for_turn(obs_turn: int) -> Tuple[int, float]:
    """
    Gentle curve floor ONLY applied when healthy (`hp_total >= 31` and board is populated).
    Prevents stalling on Tier 3-4 at 40+ HP while never forcing naked leveling at low/mid HP.
    """
    if obs_turn <= 2:
        return 1, 0.0
    if obs_turn <= 5:
        return 2, 4.5
    if obs_turn <= 8:
        return 3, 5.5
    if obs_turn <= 10:
        return 4, 6.5
    if obs_turn <= 12:
        return 5, 7.5
    return 6, 6.5


class ApexEvaluator:
    """
    Scores Tavern states by combining:
      1. 1-Turn (and Multi-Turn for Trinkets) Exact Rust Engine Physics Rollouts,
      2. CVaR Zero-Death Survival Shield + Stage-Scaled Lobby Knockout Pressure,
      3. Tribe-Gated Compounding Engine x Fuel Macro Prior + Curve Discipline + Tech Valuation,
      4. Learned Dual-Head Set-Transformer (`ApexValueNet`).
    """

    def __init__(
        self,
        catalog_name: str = "full",
        use_physics_rollout: bool = True,
        use_neural_value: bool = True,
        neural_weight: float = 0.22,
        weights_path: Optional[str] = None,
        explore_tribe: Optional[str] = None,
        explore_tier_bias: float = 0.0,
    ) -> None:
        self.catalog_name = catalog_name
        self.kb: ApexKnowledge = get_apex_knowledge(catalog_name)
        self.meta = self.kb.meta
        self.use_physics_rollout = use_physics_rollout
        self.use_neural_value = use_neural_value
        self.neural_weight = neural_weight
        self.explore_tribe = explore_tribe
        self.explore_tier_bias = explore_tier_bias
        self.predictor: Optional[ApexValuePredictor] = (
            get_apex_predictor(catalog_name=catalog_name, weights_path=weights_path)
            if use_neural_value
            else None
        )

        # Pre-bind card IDs & trinket/hero IDs for zero-allocation hot loops
        self._brann_id = self.kb.name_to_id.get("Brann Bronzebeard", -1)
        self._drakkari_id = self.kb.name_to_id.get("Drakkari Enchanter", -1)
        self._titus_id = self.kb.name_to_id.get("Titus Rivendare", -1)
        self._murkeye_id = self.kb.young_murk_eye_id
        self._poet_id = self.kb.persistent_poet_id
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

        self._hand_trinket_ids: FrozenSet[int] = frozenset(
            tid
            for tid, (tname, _, _, _) in self.kb.trinket_info.items()
            if tname in ("Nerglish Phrasebook", "Dramaloc Sticker", "Bluegill Flippers", "Flighty Portrait")
        )
        self._voone_hero_ids: FrozenSet[int] = frozenset(
            hid for hid, hname in self.kb.hero_names.items() if hname == "Rock Master Voone"
        )
        self._trinket_prior_by_id: Dict[int, float] = {
            tid: TRINKET_EQUITY_PRIORS.get(tname, 3.5)
            for tid, (tname, _, _, _) in self.kb.trinket_info.items()
        }

        # Precompute whether each ally-gated or tribal-synergy card has its own required tribe on its template
        self._support_self_has_tribe: Dict[int, bool] = {}
        for cid, req_tr in self.kb.required_ally_by_id.items():
            prof = self.meta.profile(cid)
            has_tr = bool(prof and (req_tr in prof.tribes or "all" in prof.tribes))
            self._support_self_has_tribe[cid] = has_tr
        for cid, syn_tr in self.kb.tribal_synergy_by_id.items():
            prof = self.meta.profile(cid)
            has_tr = bool(prof and (syn_tr in prof.tribes or "all" in prof.tribes))
            self._support_self_has_tribe[cid] = has_tr

        # Pre-bind `player_counters` integer indices (eliminates 2.6M string dict lookups/game!)
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
        """Steep survival utility: catastrophic penalty for death, risk-averse near lethal cap."""
        if hp_after <= 0.0:
            return -195.0 - 14.0 * max(0, alive_units - 2)

        base = 30.0 * math.log1p(hp_after / 7.0)
        next_danger = float(damage_cap) if damage_cap is not None else 18.0
        if hp_after <= next_danger:
            deficit = (next_danger + 1.0 - hp_after) / max(5.0, next_danger)
            base -= 14.0 * (deficit ** 1.4)

        if lobby_hps:
            s_hps = sorted(lobby_hps)
            median_hp = float(s_hps[len(s_hps) // 2])
            rel = (hp_after - median_hp) / max(10.0, median_hp)
            base += 5.0 * math.tanh(rel)
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
    ) -> float:
        """
        Synergy-aware compounding engine bonus:
          - Zeroes one-shot/spent carries (`Captain Sanders` once on board, `Tyrael` once >= 5 allies >= 50/50!).
          - Zeroes conditional multipliers (`Young Murk-Eye`, `Drakkari`, `Titus`) on Turn 9+ when 0 matching
            Battlecry / End-of-Turn / Deathrattle triggers exist on board!
          - Zeroes pure support minions (`Nadina`, `Soulkeeping Jailer`, `Cage Gnawer`, etc.) when 0 allies exist.
          - Discounts off-tribe carries (`Goldrinn` w/ 0 Beasts, `Kalecgos` w/ 0 Dragons, etc.) on Turn 9+,
            completely preventing mid/late-game engine churn while keeping early Turn <= 8 pivots alive!
        """
        w = self.kb.engine_bonus_by_id.get(card_id, 0.0)
        if w <= 0.0:
            return 0.0
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
                    return 0.35 * w
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
                        return 0.35 * w
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
                    return 0.35 * w
            elif card_id == self._brann_id and on_board and turn >= 10:
                bc_allies = sum(
                    1
                    for bu in board_units
                    if bu.card_id != self._brann_id
                    and (p := profiles.get(bu.card_id)) is not None
                    and p.has_battlecry
                )
                if bc_allies == 0 and tribe_counts.get("murloc", 0) < 2 and tribe_counts.get("dragon", 0) < 3:
                    return 7.0
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

        syn_tr = self.kb.tribal_synergy_by_id.get(card_id)
        if syn_tr is not None:
            allies = tribe_counts.get(syn_tr, 0) + tribe_counts.get("all", 0)
            if on_board and self._support_self_has_tribe.get(card_id, False):
                other_allies = allies - 1 if allies > 0 else 0
            else:
                other_allies = allies
            if other_allies >= 1:
                boosted = w + 0.50 * (other_allies if other_allies < 4 else 4)
                if w < 12.0 and boosted >= 11.8:
                    return 11.8
                return boosted
            # Zero matching allies currently on board:
            if turn <= 8:
                return 0.78 * w if w >= 12.0 else 0.45 * w
            if turn == 9:
                return 0.35 * w if w >= 12.5 else 0.0
            return 0.0

        return w

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
    ) -> float:
        atk = float(u.attack if u.attack > 0 else 0)
        hp = float(u.health if u.health > 0 else 0)
        val = _concave_stat_value(atk, hp) + 0.95 * u.tavern_tier

        if u.divine_shield:
            val += 3.2 + 0.36 * (atk if atk < 90.0 else 90.0)
        if u.reborn:
            val += 2.0 + 0.25 * u.tavern_tier
        if u.venomous:
            ven_base = 7.5 + 0.52 * (turn if turn < 15 else 15) + (6.5 if u.divine_shield else 0.0)
            if u.card_id == self._morgl_id:
                # `Tide Oracle Morgl` is Poisonous (never wears off!) and steals killed giant's max stats!
                ven_base += 18.0 + (22.0 if (enemy_has_giant or colossus_count >= 1) else 0.0)
            if enemy_has_giant:
                ven_base += 10.0
            if colossus_count >= 2:
                ven_base += 36.0 + 13.0 * min(4, colossus_count)
                if u.reborn or u.divine_shield:
                    ven_base += 24.0
                if u.reborn and u.divine_shield:
                    ven_base += 18.0
            val += ven_base
        if u.card_id == self._leeroy_id:
            lr_base = 7.5 + 0.48 * (turn if turn < 15 else 15) + (9.5 if enemy_has_giant else 0.0)
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
                val += 3.5 + 0.85 * u.tavern_tier

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
        apex_eng = self.effective_engine_bonus(
            u.card_id,
            turn,
            tribe_counts,
            on_board=on_board,
            board_units=board_units,
            is_golden=u.is_golden,
            obs_trinkets=obs_trinkets,
        )
        if apex_eng > 0.0:
            gold_mult = 1.75 if u.is_golden else 1.0
            val += 0.85 * apex_eng * horizon * gold_mult
        if u.card_id == self._last_standing_id:
            if u.windfury:
                val += 18.0
            if u.divine_shield:
                val += 12.0

        if prof is not None:
            val += 0.70 * prof.engine_weight * horizon * (1.55 if u.is_golden else 1.0)
            if prof.has_cleave:
                val += 0.42 * (atk if atk < 90.0 else 90.0) + (3.5 if enemy_has_summoners else 0.0)
            if (
                turn >= 7
                and u.tavern_tier <= 2
                and apex_eng <= 0.0
                and not prof.is_scaling_engine
                and (atk + hp) < 22
                and not u.venomous
            ):
                val -= 0.70 * (turn - 6)

            exp_tr = self.explore_tribe
            for tr in prof.tribes:
                if exp_tr is not None and tr == exp_tr:
                    val += 2.2
                cnt = tribe_counts.get(tr, 0)
                if cnt >= 2:
                    c_cap = cnt - 1 if cnt < 6 else 5
                    val += 0.50 * c_cap
        return val

    def unit_keep_priority(
        self,
        u: seaglass.Unit,
        turn: int,
        tribe_counts: Dict[str, int],
        scam_count: int = 0,
        particles: Optional[ApexOpponentParticles] = None,
        board_units: Optional[Sequence[seaglass.Unit]] = None,
        obs_trinkets: Optional[Sequence[int]] = None,
    ) -> float:
        """Priority for keeping a minion on board (lowest priority = best cycle/replacement slot)."""
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
        )
        apex_eng = self.effective_engine_bonus(
            u.card_id,
            turn,
            tribe_counts,
            on_board=True,
            board_units=board_units,
            is_golden=u.is_golden,
            obs_trinkets=obs_trinkets,
        )
        if apex_eng >= 10.0:
            h_keep = (16.5 - turn) / 9.0
            h_clamp = 0.45 if h_keep < 0.45 else (1.30 if h_keep > 1.30 else h_keep)
            score += (30.0 if apex_eng >= 12.0 else 19.0) * h_clamp

        # Protect scarce Venomous / Leeroy / Morgl scam units on Turn 9+ so we NEVER sell them into colossi!
        max_scam_keep = 5 if eff_col >= 3 else (3 if enemy_giant else 2)
        if turn >= 9 and (u.venomous or u.card_id in (self._leeroy_id, self._morgl_id)) and scam_count <= max_scam_keep:
            score += 48.0 if eff_col >= 3 else (28.0 if enemy_giant else 18.0)

        if u.card_id in self.kb.true_on_sell_plain_ids and apex_eng < 8.0 and not u.venomous:
            score -= 8.5
        elif u.card_id in self.kb.golden_only_sell_ids and u.is_golden and not u.venomous:
            score -= 18.0
        return score

    def macro_and_economy_value(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = True,
        particles: Optional[ApexOpponentParticles] = None,
    ) -> float:
        turn = obs.turn if obs.turn > 1 else 1
        hp_total = self.effective_hp(obs)
        if hp_total < 1.0:
            hp_total = 1.0
        dmg_cap = ctx.damage_cap if ctx is not None else None
        alive_units = ctx.alive_units if ctx is not None else 8
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

        # Direct concave EHP survival value so Tavern self-damage (`Lich Baz'hial`, health-cost cards)
        # is accurately priced on EVERY beam node (not just at end-of-turn combat rollouts)!
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
            )
            score += uv
            if uv < weakest_board_score:
                weakest_board_score = uv
        if not board:
            weakest_board_score = 0.0
        if has_last_standing > 0:
            distinct_tr = min(7, len(tribe_counts) + 2 * tribe_counts.get("all", 0))
            score += 4.5 * has_last_standing * distinct_tr * safety

        # Anti-synergy guard: `Boom-in-a-Box` deals 3 AoE to friendly minions at Start of Combat, popping our own shields!
        if boom_count > 0 and ds_count >= 2:
            score -= 7.5 * ds_count

        # Tempo slot presence bonus + strict full-board discipline on Turns 7+!
        n_board = len(board)
        slot_w = 3.0 if turn <= 6 else 2.5
        score += slot_w * (n_board if n_board < 7 else 7)
        if end_of_turn and turn >= 7 and n_board < 7:
            score -= 11.5 * (7 - n_board)

        # 1. Coherent tribe & multiplicative engine synergies
        if tribe_counts:
            dom_cnt = max(tribe_counts.values())
            if dom_cnt >= 3:
                score += 1.35 * ((dom_cnt - 2) ** 1.45) * safety

        if summoners > 0 and (combat_obs > 0 or has_titus > 0):
            score += 2.8 * min(4, summoners) * min(5, combat_obs + 2 * has_titus) * safety
        if has_drakkari > 0 and eot_engines > 0:
            score += 4.5 * has_drakkari * min(5, eot_engines) * horizon * safety
        elif eot_engines >= 2:
            score += 1.9 * (eot_engines - 1) * horizon * safety
        if (bc_engines > 0 or has_brann > 0 or has_murkeye > 0) and battlecries > 0:
            score += (
                2.8
                * (bc_engines + 2 * has_brann + 3 * has_murkeye)
                * min(4, battlecries)
                * horizon
                * safety
            )
        if has_poet > 0 and tribe_counts.get("dragon", 0) >= 2:
            score += 4.0 * has_poet * min(4, tribe_counts.get("dragon", 0) - 1) * horizon * safety
        if mag_engines > 0 and tribe_counts.get("mech", 0) >= 2:
            score += 3.0 * mag_engines * min(5, tribe_counts.get("mech", 0)) * horizon * safety
        if play_engines > 0:
            score += 2.0 * play_engines * horizon * safety
        if gold_engines > 0:
            score += 2.2 * gold_engines * horizon * safety
        if spell_engines > 0:
            score += 2.0 * spell_engines * horizon * safety

        # 2. Tavern Tier curve + Natural Cheap-Upgrade Opportunity Discipline!
        tier = obs.tavern_tier
        if tier < 1:
            tier = 1
        elif tier > 7:
            tier = 7
        owns_candle = self._candle_id > 0 and any(tid == self._candle_id for tid in obs_trinkets)
        raw_th = (19.5 - turn) / 8.5
        tier_horizon = 0.62 if raw_th < 0.62 else (1.25 if raw_th > 1.25 else raw_th)
        if owns_candle and tier > 4:
            base_tv = _APEX_TIER_CURVE[4] + 0.30 * (_APEX_TIER_CURVE[tier] - _APEX_TIER_CURVE[4])
        else:
            base_tv = _APEX_TIER_CURVE[tier]
        tier_val = base_tv + self.explore_tier_bias * (tier - 1)
        if alive_units <= 2 and turn <= 6 and (hp_total < 30.0 or n_board < min(6, turn)):
            tier_val *= 0.85
        score += tier_val * tier_horizon * safety

        if turn >= 4 and tier == 1:
            score -= 6.5 * (turn - 2)

        # Solo & Duos Cheap-Upgrade Opportunity Discipline:
        # Never naked-double-level at 9-11g AND never stall on Tier 3/4 on Turns 8-13 when healthy!
        is_duos = hasattr(obs, "team_health")
        cheap_upg = 7 if is_duos else 6
        if is_duos and getattr(obs, "teammate_tavern_tier", 0) > tier and obs.upgrade_cost >= 8:
            score -= 3.0
        if hp_total >= 24.0 and n_board >= 6 and not owns_candle:
            if turn >= 8 and tier < 4 and obs.upgrade_cost <= cheap_upg:
                score -= 7.5 * (4 - tier)
            elif turn >= 9 and tier < 4:
                score -= 8.0 * (4 - tier)
            elif turn >= 10 and tier < 5 and obs.upgrade_cost <= cheap_upg + 1:
                score -= 7.5
            elif turn >= 11 and tier < 5:
                score -= 8.5
            elif turn >= 12 and tier < 6 and obs.upgrade_cost <= cheap_upg + 1 and hp_total >= 26.0:
                score -= 7.0
        # Anti-Colossus Tier-5 Tech Urgency: unlock `Leeroy`, `Upper Hand` & `Bile Spitter` against colossi!
        if col_cnt >= 3 and tier < 5 and hp_total >= 16.0 and n_board >= 6 and not owns_candle:
            score -= 14.0 * (5 - tier)

        # `Weighted Gauntlet` Mastery (Lesser & Greater): multiply exponential EoT buffs with Drakkari + DS/Cleave/Venomous!
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
            nominal_upg = tier + 5
            discount = nominal_upg - int(obs.upgrade_cost)
            if discount > 0:
                score += 0.65 * discount * horizon * safety

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
                            c, turn, tribe_counts, enemy_giant, enemy_sum, on_board=False, colossus_count=col_cnt, enemy_has_ds=enemy_ds, board_units=board, obs_trinkets=obs_trinkets
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
                            c, turn, tribe_counts, enemy_giant, enemy_sum, on_board=False, colossus_count=col_cnt, enemy_has_ds=enemy_ds, board_units=board, obs_trinkets=obs_trinkets
                        )
                        score += max(2.2 + 0.50 * c.tavern_tier, 0.85 * cand_val) * safety
                    elif hand_synergy and h_idx == 0:
                        score += (1.8 + 0.45 * _concave_stat_value(c.attack, c.health)) * safety
                    else:
                        # Bellman-consistent 1-step replacement value (`Sell(weakest) -> Play(c)`)!
                        cand_val = self.unit_value(
                            c, turn, tribe_counts, enemy_giant, enemy_sum, on_board=False, colossus_count=col_cnt, enemy_has_ds=enemy_ds, board_units=board, obs_trinkets=obs_trinkets
                        )
                        swap_gain = cand_val - weakest_board_score + 1.80
                        if swap_gain < 0.0:
                            swap_gain = 0.0
                        bc_bonus = 2.6 if (prof is not None and prof.has_battlecry) else 0.0
                        c_eng = self.effective_engine_bonus(cid, turn, tribe_counts, on_board=False, board_units=board, is_golden=c.is_golden, obs_trinkets=obs_trinkets)
                        base_hold = 0.65 + 0.30 * c.tavern_tier + 0.35 * c_eng
                        score += max(base_hold, 0.88 * swap_gain + bc_bonus) * safety

        # 4. Pair potential (capped so bot doesn't hoard 4 pairs)
        pairs_counted = 0
        tmpls = self.meta.templates_by_id
        for cid, cnt in all_copy_counts.items():
            if cnt == 2 and pairs_counted < 2:
                tmpl = tmpls.get(cid)
                t_val = tmpl.tavern_tier if tmpl else 1
                eng_b = self.effective_engine_bonus(cid, turn, tribe_counts, on_board=True)
                if turn <= 6 or t_val >= max(2, tier - 1) or eng_b >= 8.0 or board_copy_counts.get(cid, 0) == 2:
                    pairs_counted += 1
                    score += (1.5 + 0.40 * min(6, tier + 1) + 0.30 * t_val + 0.15 * eng_b) * safety

        # 5. Freeze discipline
        if obs.is_frozen:
            score += self._freeze_shop_value(obs, all_copy_counts, tribe_counts) * safety

        # 6. Persistent Player Counters (direct integer indexing!), Deity, Card Counters & Trinkets
        score += 1.85 * obs.bonus_gold_next_turn * safety
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

        # Next-combat spell effects (`Upper Hand`, `Boon of Beetles`, `Defender's Rites`, etc.)
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

        # 7. Bellman Purchasing & Roll Continuation Power on interior nodes!
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
    ) -> float:
        if obs.gold >= obs.minion_cost:
            return -2.2
        tier = obs.tavern_tier
        turn = obs.turn
        if tribe_counts is None:
            tribe_counts = {}
            for u in obs.board:
                for tr in u.tribes:
                    tribe_counts[tr] = tribe_counts.get(tr, 0) + 1
        best_fz = -2.0
        for s in obs.shop:
            if s.is_spell:
                if s.spell_cost >= 2 and obs.gold < s.spell_cost and turn <= 8:
                    best_fz = max(best_fz, 0.5)
                continue
            eng_b = self.effective_engine_bonus(s.card_id, turn, tribe_counts, on_board=False)
            if not s.is_golden and all_copies.get(s.card_id, 0) >= 2:
                invested_stats = sum(
                    u.attack + u.health for u in obs.board if u.card_id == s.card_id
                )
                if s.tavern_tier >= max(1, tier - 2) or turn <= 7 or invested_stats >= 22 or eng_b >= 8.0:
                    best_fz = max(best_fz, 2.6)
            else:
                owns_candle = self._candle_id > 0 and any(tid == self._candle_id for tid in obs.trinkets)
                if eng_b >= 12.0 and s.tavern_tier >= max(4, tier):
                    best_fz = max(best_fz, 2.4)
                elif not owns_candle and eng_b >= 9.0 and s.tavern_tier >= max(4, tier):
                    best_fz = max(best_fz, 1.8)
                elif not owns_candle and s.tavern_tier >= max(4, tier + 1) and eng_b > 0.0:
                    best_fz = max(best_fz, 1.2)
        return best_fz

    def evaluate_fast_batch(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = False,
        particles: Optional[ApexOpponentParticles] = None,
        in_dist_sot: bool = False,
    ) -> List[float]:
        if not obs_list:
            return []
        prior_scores = [
            self.macro_and_economy_value(o, ctx=ctx, end_of_turn=end_of_turn, particles=particles)
            for o in obs_list
        ]
        if (
            not (in_dist_sot or (end_of_turn and len(obs_list) >= 2))
            or not self.has_neural_model
            or self.predictor is None
            or (ctx is not None and (getattr(ctx, "duos", False) or ctx.alive_units <= 2))
        ):
            return prior_scores

        net_scores = self.predictor.predict_batch(obs_list, ctx=ctx, end_of_turn=end_of_turn)
        w = self.neural_weight
        return [p_val + w * n_val for p_val, n_val in zip(prior_scores, net_scores)]

    def evaluate_fast(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = False,
        particles: Optional[ApexOpponentParticles] = None,
        in_dist_sot: bool = False,
    ) -> float:
        return self.evaluate_fast_batch(
            [obs],
            ctx=ctx,
            end_of_turn=end_of_turn,
            particles=particles,
            in_dist_sot=in_dist_sot,
        )[0]

    def _pre_end_turn_discipline(self, obs: seaglass.TavernObservation) -> float:
        adj = 0.0
        hp_total = max(1.0, self.effective_hp(obs))
        safety = min(1.05, max(0.22, hp_total / 20.0))
        hand_syn = self.has_hand_buff_synergy(obs)

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
            adj += self._freeze_shop_value(obs, all_copies, tribe_counts) * safety
        return adj

    def evaluate_turn_end(
        self,
        tavern: seaglass.TavernGame,
        particles: ApexOpponentParticles,
        ctx: Optional[seaglass.LobbyContext] = None,
        combats_per_particle: int = 14,
        board_override: Optional[List[seaglass.Unit]] = None,
    ) -> float:
        """
        Full 1-Turn Physics + CVaR Survival Evaluation:
          1. Step `end_turn()` on a copy so ALL Rust End-of-Turn triggers fire before combat.
          2. Run Monte Carlo `equity_against` on regular particles + CVaR highroll check.
          3. Step `resolve_combat_against` + `start_turn()` -> evaluate Turn `t + 1` macro state.
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

        # Preserve full midgame tempo safety (`ko_mult = 1.0`) so weak scaling villains are knocked out early!
        ko_mult = 1.65 if is_endgame else 1.0

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
                offense_bonus_sum += ko_mult * (0.30 * dealt + 2.9 * dist.a_win_rate)

        # CVaR Highroll Shield: check 90th-percentile opponent spike when cap is lifted or HP <= cap + 6!
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

        # Colossus-Boss Readiness Check: ONLY when a verified exponential/colossus boss (`lobby_colossus_count >= 2`)
        # is alive in the lobby (and we don't own Gauntlet ourselves), simulate combat against `lobby_boss_board`
        # so we proactively assemble Venomous/Leeroy/DS scam BEFORE the 1v1 showdown!
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
            combat_icm_score += 0.30 * boss_bonus

        survival_prob = max(0.0, min(1.0, 1.0 - (lethal_prob_sum / n_boards)))

        # Step 3: 1-Turn Engine Physics Rollout -> Turn `t + 1` Start-of-Turn Macro Evaluation
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
                g_next.start_turn()
                next_obs_samples.append(g_next.observe())

            next_scores = self.evaluate_fast_batch(
                next_obs_samples,
                ctx=ctx,
                end_of_turn=False,
                particles=particles,
                in_dist_sot=True,
            )
            avg_next_macro = sum(next_scores) / len(next_scores)
            pre_discipline = self._pre_end_turn_discipline(obs_pre)
            continuation_weight = 0.20 + 0.80 * survival_prob
            macro_score = continuation_weight * avg_next_macro + pre_discipline
        else:
            macro_score = self.evaluate_fast(
                obs_pre, ctx=ctx, end_of_turn=True, particles=particles
            )

        return combat_icm_score + macro_score

    def choose_best_hero(
        self,
        obs: seaglass.TavernObservation,
        legal: Sequence[seaglass.TavernAction],
        base_game: Optional[seaglass.TavernGame] = None,
    ) -> seaglass.TavernAction:
        """Select offered Hero using calibrated multi-turn equity priors + starting EHP."""
        best_act = legal[0]
        best_score = -1e9
        for act in legal:
            if act.kind != "choose_hero":
                continue
            opt_idx, _ = act.args
            if opt_idx is None or opt_idx >= len(obs.hero_options):
                continue
            hid = obs.hero_options[opt_idx]
            hname = self.kb.hero_names.get(hid, "")
            score = 0.0
            if hid in self.kb.hero_implemented:
                score += 3.0
            score += HERO_EQUITY_PRIORS.get(hname, 3.5)

            prof = self.meta.profiles.get(hid)
            if prof is not None:
                score += 0.65 * prof.engine_weight
                if prof.is_economy_generator:
                    score += 2.0

            if base_game is not None:
                sim = base_game.copy()
                if sim.is_legal(act):
                    sim.step(act)
                    s_obs = sim.observe()
                    ehp = self.effective_hp(s_obs)
                    score += 0.32 * (ehp - 30.0)
                    if self.has_neural_model:
                        score += 0.35 * self.evaluate_fast(s_obs, end_of_turn=True)
            if score > best_score:
                best_score = score
                best_act = act
        return best_act
