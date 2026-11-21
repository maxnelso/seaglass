"""Build-Aware + 1-Turn Rust Physics Evaluator (`ArchitectEvaluator`).

Combines:
  1. Fast build-progress & engine-synergy evaluation (`evaluate_fast`)
  2. Exact 1-turn Rust physics rollouts (`evaluate_turn_end`: comp-position -> `end_turn()` -> `equity_against`)
  3. Comp-conditioned Hero, Trinket, Discover, and Dark Gift scoring
  4. ICM-style HP/survival weighting (greed when healthy/ghost, stabilize/scam when low HP or top-2)
"""

from __future__ import annotations

import math
from typing import Dict, FrozenSet, List, Optional, Sequence, Tuple

import seaglass

from ..sovereign_belief import SovereignOpponentParticles
from ..sovereign_knowledge import SovereignKnowledge, get_sovereign_knowledge
from ..sovereign_model import SovereignNumpyPredictor, get_sovereign_predictor
from .comps import CompRegistry, get_comp_registry
from .planner import BuildPlan, CompPlanner
from .positioning import ArchitectPositioner

# Calibrated 8-Player FFA Tavern Tier value curve (crisp T2-on-T2 curve + disciplined T4/T5/T6 scaling)
_TIER_BASE_VAL = (0.0, 0.0, 14.5, 24.5, 36.5, 52.5, 67.5, 80.0)

# High-value Cataclysmic Harbinger payoff spells (never overwrite with junk spells!)
_HARBINGER_PAYOFF_SPELLS: FrozenSet[str] = frozenset(
    {
        "Azerite Empowerment",
        "Menagerie Tableware",
        "Mighty Dragonbreath",
        "Wave of Gold",
        "Butchering",
        "Hallowed Ritual",
        "Misplaced Tea Set",
        "Eyes of the Earth Mother",
        "Corrupted Cupcakes",
        "Methodical Madness",
        "Blood Gem Barrage",
        "Easterly Winds",
        "Staff of Enrichment",
        "Eonar's Favor",
        "Golden Touch",
        "Upper Hand",
        "Sharing is Caring",
    }
)


def _concave_stat(x: float) -> float:
    if x <= 0.0:
        return 0.0
    if x <= 80.0:
        return x
    return 80.0 + 22.0 * math.sqrt(x - 80.0)


class ArchitectEvaluator:
    def __init__(
        self,
        catalog_name: str = "full",
        use_neural_value: bool = False,
        neural_weight: float = 0.22,
    ) -> None:
        self.catalog_name = catalog_name
        self.reg: CompRegistry = get_comp_registry(catalog_name)
        self.planner = CompPlanner(catalog_name)
        self.positioner = ArchitectPositioner(catalog_name)
        self.sov_kb: SovereignKnowledge = get_sovereign_knowledge(catalog_name)
        self.kb = self.sov_kb.base
        self.use_neural_value = use_neural_value
        self.neural_weight = neural_weight
        self.predictor: Optional[SovereignNumpyPredictor] = (
            get_sovereign_predictor(catalog_name=catalog_name) if use_neural_value else None
        )

        self._trinket_id_to_name: Dict[int, str] = {
            tid: tname for tid, tname, _g, _c, _impl in seaglass.trinkets_table()
        }
        self._c_idx = self.reg.counter_idx_by_name

        # Pairwise core combo bonuses (when BOTH cards are on board simultaneously!)
        self._pair_combos: Tuple[Tuple[int, int, float], ...] = tuple(
            (a, b, w)
            for n1, n2, w in (
                # Undead Reborn + Butcher
                ("Snazzy Phantom", "Drustfallen Butcher", 18.0),
                ("Snazzy Phantom", "Barrier Banshee", 16.0),
                ("Snazzy Phantom", "Dead Bellringer", 15.0),
                ("Drustfallen Butcher", "Barrier Banshee", 12.0),
                ("Eternal Summoner", "Eternal Tycoon", 15.0),
                ("Eternal Summoner", "Titus Rivendare", 15.0),
                ("Plaguerunner", "Dead Bellringer", 12.0),
                # Quilboar
                ("Victorious Geomant", "Hot-Air Surveyor", 22.0),
                ("Victorious Geomant", "Geomagus Roogug", 16.0),
                ("Victorious Geomant", "Balinda Stonehearth", 18.0),
                ("Sanguine Champion", "Brann Bronzebeard", 15.0),
                ("Sanguine Champion", "Titus Rivendare", 14.0),
                ("Sanguine Champion", "Young Murk-Eye", 18.0),
                ("Turbo Hogrider", "Thorned Trailblazer", 16.0),
                ("Turbo Hogrider", "Veteran Brigand", 16.0),
                # Mechs
                ("Resourceful Robot", "Drakkari Enchanter", 22.0),
                ("Utility Drone", "Drakkari Enchanter", 20.0),
                ("Resourceful Robot", "Utility Drone", 18.0),
                ("Polarizing Beatboxer", "Resourceful Robot", 24.0),
                ("Polarizing Beatboxer", "Utility Drone", 22.0),
                ("Glambot", "Balinda Stonehearth", 16.0),
                ("Charging Czarina", "Gearfin", 14.0),
                # Beasts
                ("Goldrinn, the Great Wolf", "Titus Rivendare", 22.0),
                ("Goldrinn, the Great Wolf", "Banana Slamma", 20.0),
                ("Goldrinn, the Great Wolf", "Deathstrider", 18.0),
                ("Turquoise Skitterer", "Titus Rivendare", 18.0),
                ("Turquoise Skitterer", "Deathstrider", 18.0),
                ("Turquoise Skitterer", "Ravaging Scorpid", 16.0),
                # Dragons
                ("Persistent Poet", "Hopebringer", 20.0),
                ("Persistent Poet", "Heroic Broodmother", 22.0),
                ("Persistent Poet", "Crimson Vindicator", 20.0),
                ("Persistent Poet", "Runic Arcanist", 14.0),
                ("Hopebringer", "Heroic Broodmother", 18.0),
                ("Kalecgos, Arcane Aspect", "Brann Bronzebeard", 22.0),
                # Elementals & Demons
                ("Flaming Enforcer", "Drakkari Enchanter", 18.0),
                ("En-Djinn Blazer", "Brann Bronzebeard", 18.0),
                ("Living Azerite", "Unbound Tempest", 16.0),
                ("Soul Rewinder", "Tichondrius", 15.0),
                ("Soul Rewinder", "Malchezaar, Prince of Dance", 12.0),
                ("Soul Rewinder", "Eredar Escapist", 16.0),
                ("Deft Deserter", "Insatiable Ur'zul", 14.0),
                ("Deft Deserter", "Soulkeeping Jailer", 14.0),
                # Murlocs
                ("Young Murk-Eye", "Brann Bronzebeard", 24.0),
                ("Young Murk-Eye", "Drakkari Enchanter", 20.0),
                ("Young Murk-Eye", "Hackerfin", 20.0),
                ("Young Murk-Eye", "Primalfin Lookout", 16.0),
                ("Twilight Tidehunter", "Balinda Stonehearth", 16.0),
                ("Twilight Tidehunter", "Shamanic Tidecaller", 14.0),
                ("Choral Mrrrglr", "Futurefin", 22.0),
                # Pirates
                ("Sky Admiral Rogers", "Proud Privateer", 22.0),
                ("Hooktusk, Master Marauder", "Enterprising Escapee", 16.0),
                ("Ship Master Eudora", "Titus Rivendare", 15.0),
                # Aberrations
                ("Mindbender Ghur'sha", "Cutthroat K'Thir", 15.0),
                ("Mindbender Ghur'sha", "Harbinger Aph'lass", 16.0),
                ("Mindbender Ghur'sha", "Mindbending Recruiter", 14.0),
                ("Faceless Converter", "Titus Rivendare", 16.0),
                # Spells & Menagerie
                ("Felfire Conjurer", "Drakkari Enchanter", 20.0),
                ("Cataclysmic Harbinger", "Drakkari Enchanter", 20.0),
                ("Gatekeeper Amalgam", "Balinda Stonehearth", 24.0),
                ("Gatekeeper Amalgam", "Victorious Geomant", 18.0),
                ("Felfire Conjurer", "Cataclysmic Harbinger", 16.0),
            )
            if (a := self.reg.name_to_id.get(n1)) is not None
            and (b := self.reg.name_to_id.get(n2)) is not None
        )

        self._boom_id = self.reg.name_to_id.get("Boom-in-a-Box", -1)
        self._leeroy_id = self.reg.name_to_id.get("Leeroy the Reckless", -1)
        self._poet_id = self.reg.name_to_id.get("Persistent Poet", -1)
        self._murkeye_id = self.reg.name_to_id.get("Young Murk-Eye", -1)
        self._bream_id = self.reg.name_to_id.get("Bream Counter", -1)
        self._scout_id = self.reg.name_to_id.get("Flighty Scout", -1)
        self._choral_id = self.reg.name_to_id.get("Choral Mrrrglr", -1)
        self._costume_id = self.reg.name_to_id.get("Costume Enthusiast", -1)
        self._diremuck_id = self.reg.name_to_id.get("Diremuck Forager", -1)
        self._futurefin_id = self.reg.name_to_id.get("Futurefin", -1)
        self._candle_id = self.kb.guiding_candle_id
        self._non_stacking_aura_ids: FrozenSet[int] = frozenset(
            cid
            for name in ("Brann Bronzebeard", "Drakkari Enchanter", "Soul Rewinder")
            if (cid := self.reg.name_to_id.get(name)) is not None
        )

    def score_hero(self, hero_id: int, lobby_tribes: Sequence[str]) -> float:
        """Score a hero offer conditioned on the 5-tribe lobby."""
        return self.sov_kb.hero_lobby_score(hero_id, lobby_tribes)

    def score_trinket(
        self,
        trinket_id: int,
        cost: int,
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
    ) -> float:
        """Score a trinket offer combining empirical lobby strength + comp synergy - gold cost."""
        tname = self._trinket_id_to_name.get(trinket_id, "")
        base = self.sov_kb.trinket_prior_score(tname)

        # Comp affinity bonus: if this trinket directly supercharges our primary or top comp!
        comp_bonus = 0.0
        if plan.primary is not None:
            tw = plan.primary.comp.trinkets.get(tname, 0.0)
            if tw > 0.0:
                comp_bonus += tw * (0.45 + 0.75 * plan.commitment)
        if plan.secondary is not None and plan.commitment < 0.75:
            tw2 = plan.secondary.comp.trinkets.get(tname, 0.0)
            if tw2 > 0.0:
                comp_bonus += tw2 * 0.25

        # Penalize unaffordable / gold-draining trinkets slightly relative to remaining gold
        gold_pen = 1.65 * float(cost)
        if cost > obs.gold:
            gold_pen += 50.0
        return base + comp_bonus - gold_pen

    def _board_power(
        self,
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
        turn_horizon: float,
    ) -> float:
        val = 0.0
        board = obs.board
        board_ids = set()
        tribes_seen = set()
        tribe_counts: Dict[str, int] = {}
        for u in board:
            tr_u = str(u.tribe).lower()
            if tr_u == "all":
                for k_tr in (
                    "aberration",
                    "beast",
                    "demon",
                    "dragon",
                    "elemental",
                    "mech",
                    "murloc",
                    "pirate",
                    "quilboar",
                    "undead",
                ):
                    tribe_counts[k_tr] = tribe_counts.get(k_tr, 0) + 1
            elif tr_u and tr_u != "none":
                for part in tr_u.split("_"):
                    tribe_counts[part] = tribe_counts.get(part, 0) + 1

        seen_non_stack: Dict[int, bool] = {}  # card_id -> already have a Golden or 1st copy
        bonus_keywords_seen = set()
        has_boom = False
        has_leeroy = False
        has_hand_payoff = False

        for u in board:
            cid = u.card_id
            board_ids.add(cid)
            if cid == self._boom_id:
                has_boom = True
            elif cid == self._leeroy_id:
                has_leeroy = True
            if cid in (self._choral_id, self._costume_id, self._diremuck_id, self._futurefin_id):
                has_hand_payoff = True

            # 1. Raw combat stats (concave past 80 so we don't ignore DS/scam/multipliers)
            a = float(max(0, u.attack))
            h = float(max(0, u.health))
            stat_v = 0.48 * _concave_stat(a) + 0.48 * _concave_stat(h)

            # Keyword combat multipliers
            if u.divine_shield:
                stat_v += 4.5 + 0.28 * _concave_stat(a)
                bonus_keywords_seen.add("ds")
            if u.windfury:
                stat_v += 2.2 + 0.14 * _concave_stat(a)
                bonus_keywords_seen.add("wf")
            if u.reborn:
                stat_v += 3.2 + 0.18 * _concave_stat(a)
                bonus_keywords_seen.add("rb")
            if u.venomous:
                stat_v += 9.5 + (6.0 if u.divine_shield else 0.0) + (5.0 if u.reborn else 0.0)
                bonus_keywords_seen.add("vn")
            if u.taunt:
                stat_v += 0.6
                bonus_keywords_seen.add("tn")
            if u.stealth:
                stat_v += 1.5
                bonus_keywords_seen.add("st")
            if cid in self.reg.cleave_ids:
                stat_v += 5.5 + 0.35 * _concave_stat(a)
            if u.is_golden:
                stat_v += 3.5 + 1.0 * u.tavern_tier
            if u.dark_gifts:
                for dgid in u.dark_gifts:
                    stat_v += 0.55 * self.kb.dark_gift_weights.get(dgid, 4.0)

            val += stat_v

            # Check tribal ally requirement for tribe-dependent engines (e.g. Crimson Vindicator with 0 other Dragons!)
            req_tr = self.kb.tribal_synergy_by_id.get(cid) or self.kb.required_ally_by_id.get(cid)
            ally_mult = 1.0
            if req_tr is not None:
                # Subtract self if self matches req_tr
                self_matches = req_tr in str(u.tribe).lower() or str(u.tribe).lower() == "all"
                other_allies = tribe_counts.get(req_tr, 0) - (1 if self_matches else 0)
                if other_allies <= 0:
                    ally_mult = 0.0 if obs.turn >= 9 else 0.45
                elif other_allies >= 2:
                    ally_mult = 1.15

            # Non-stacking duplicate aura discount (e.g. Golden Soul Rewinder + plain Soul Rewinder on board!)
            dup_mult = 1.0
            if cid in self._non_stacking_aura_ids:
                if cid in seen_non_stack:
                    # Second copy on board doesn't stack its passive!
                    dup_mult = 0.0 if (seen_non_stack[cid] or u.is_golden) else 0.25
                    val -= 6.0 if (seen_non_stack[cid] or u.is_golden) else 0.0
                else:
                    seen_non_stack[cid] = bool(u.is_golden)

            # 2. Compounding engine & comp-fit value (discounted by remaining turns)
            eng_base = self.kb.engine_bonus_by_id.get(cid, 0.0)
            if eng_base > 0.0:
                n_mult = self.sov_kb.neutral_engine_tribe_multiplier(cid, tuple(plan.lobby_tribes))
                g_mult = 1.65 if u.is_golden else 1.0
                val += eng_base * n_mult * g_mult * ally_mult * dup_mult * 0.65 * turn_horizon

            # Comp-specific synergy score
            cname = u.name or self.reg.id_to_name.get(cid, "")
            if plan.primary is not None:
                w, role = plan.comp_card_weight(plan.primary.comp, cid, cname)
                if w > 0.0:
                    g_mult = 1.50 if u.is_golden else 1.0
                    role_mult = (
                        1.15
                        if role in ("lynchpin", "multiplier")
                        else (0.90 if role == "scaler" else 0.45)
                    )
                    val += w * role_mult * g_mult * ally_mult * dup_mult * (0.30 + 0.70 * plan.commitment) * turn_horizon
            elif obs.turn >= 4:
                for cs in plan.ranked[:4]:
                    w, role = plan.comp_card_weight(cs.comp, cid, cname)
                    if role == "lynchpin" and w >= 12.0:
                        val += w * ally_mult * 0.45 * turn_horizon
                        break

            # Track tribe diversity for Menagerie
            tr = str(u.tribe).lower()
            if tr and tr != "none":
                tribes_seen.add(tr)

        # 3. Pairwise multi-card engine completion bonuses!
        for c1, c2, bonus in self._pair_combos:
            if c1 in board_ids and c2 in board_ids:
                val += bonus * turn_horizon

        # 4. Menagerie tribe diversity & Keyword-soup Hackerfin bonuses
        if "maximize_tribes" in plan.active_rules:
            val += 4.5 * len(tribes_seen) * turn_horizon
        if "spread_bonus_keywords" in plan.active_rules:
            val += 3.5 * len(bonus_keywords_seen) * turn_horizon

        # 5. Anti-synergy / invariant penalties
        if has_boom and has_leeroy:
            val -= 24.0  # Boom-in-a-Box kills our own Leeroy at Start of Combat!
        if has_boom:
            own_ds_cnt = sum(1 for u in board if u.divine_shield and u.card_id != self._boom_id)
            if own_ds_cnt >= 2:
                val -= 8.0 * own_ds_cnt  # Boom-in-a-Box pops all our own Divine Shields!
        if self._poet_id in board_ids:
            dragon_count = sum(
                1
                for u in board
                if u.card_id != self._poet_id
                and ("dragon" in str(u.tribe).lower() or str(u.tribe).lower() == "all")
            )
            if dragon_count == 0:
                val -= 10.0

        # 6. Hand value (triples, handbuff Murlocs, Bream Counter, unplayed spells, Lockbox)
        #    CRITICAL: NEVER value holding `Triple Reward` higher than casting it at Tier 4+,
        #    and penalize hand-clogging (>= 7 non-handbuff cards) so the bot never hoards 10 cards!
        hand_val = 0.0
        for c in obs.hand:
            if c.is_spell:
                if c.name == "Lockbox":
                    hand_val += 11.0 * turn_horizon
                elif c.name == "Triple Reward":
                    # Only modest hold value at T1-T3 (to level before discovering); at T4+ casting it is strictly better!
                    hand_val += (14.0 if obs.tavern_tier <= 3 else 4.0)
                elif c.name == "Blood Gem":
                    bg_a = obs.player_counters[self._c_idx.get("blood_gem_bonus_atk", 8)]
                    bg_h = obs.player_counters[self._c_idx.get("blood_gem_bonus_hp", 9)]
                    hand_val += 1.0 + 0.25 * (bg_a + bg_h)
                else:
                    hand_val += 0.8
            else:
                # Minion in hand: worth little unless it's a Handbuff Murloc target or Golden!
                c_stat = 0.35 * _concave_stat(c.attack) + 0.35 * _concave_stat(c.health)
                if has_hand_payoff or c.card_id in (self._bream_id, self._scout_id):
                    hand_val += max(6.0, 0.85 * c_stat) + (8.0 if c.card_id == self._bream_id else 0.0)
                else:
                    # Only slight resale/pair value — never incentivize buying trash just to hold in hand!
                    hand_val += 0.6 + (4.5 if c.is_golden else 0.0)

        if not has_hand_payoff and len(obs.hand) >= 7:
            hand_val -= 2.2 * (len(obs.hand) - 6)
        if len(obs.hand) >= 9:
            hand_val -= 8.0 * (len(obs.hand) - 8)

        # Pair-to-triple value across board + hand (especially for core comp pieces!)
        ng_counts: Dict[int, int] = {}
        for u in list(obs.board) + [c for c in obs.hand if not c.is_spell]:
            if not u.is_golden:
                ng_counts[u.card_id] = ng_counts.get(u.card_id, 0) + 1
        for cid, cnt in ng_counts.items():
            if cnt == 2:
                pair_v = 3.5 + 0.22 * plan.card_desire(cid)
                hand_val += pair_v * turn_horizon

        # 7. Persistent scaling counters (rest-of-game investment!)
        ctrs = obs.player_counters
        def _c(name: str) -> float:
            idx = self._c_idx.get(name)
            return float(ctrs[idx]) if idx is not None and idx < len(ctrs) else 0.0

        counter_val = 0.0
        undead_on_board = sum(1 for u in board if "undead" in str(u.tribe).lower() or str(u.tribe).lower() == "all")
        qb_on_board = sum(1 for u in board if "quilboar" in str(u.tribe).lower() or str(u.tribe).lower() == "all")
        ab_on_board = sum(1 for u in board if "aberration" in str(u.tribe).lower() or str(u.tribe).lower() == "all")

        # Global Undead Attack/Health (massive with Reborn Undead + Butcher + Snazzy!)
        if undead_on_board > 0 or plan.primary_id.startswith("undead"):
            counter_val += (0.85 * _c("undead_bonus_attack") + 1.15 * _c("undead_bonus_health")) * (
                1.0 + 0.18 * undead_on_board
            )
        # Global Blood Gem scaling
        if qb_on_board > 0 or plan.primary_id.startswith("quilboar"):
            counter_val += 1.65 * (_c("blood_gem_bonus_atk") + _c("blood_gem_bonus_hp"))
        # Global Tavern Spell Power
        counter_val += 1.85 * (_c("spell_bonus_atk") + _c("spell_bonus_hp"))
        # Global Shop Buffs (Eat-the-Tavern / Elementals)
        shop_buf = (
            _c("tavern_all_atk")
            + _c("tavern_all_hp")
            + 0.75 * (_c("tavern_elemental_atk") + _c("tavern_elemental_hp"))
            + 0.70 * (_c("demon_tavern_atk") + _c("demon_tavern_hp"))
            + 2.2 * _c("refresh_buffs")
            + 2.8 * _c("refresh_blood_gems")
        )
        counter_val += 0.65 * shop_buf
        # Aberration Deity stats (real extra body in combat once 4 Aberrations die!)
        if ab_on_board >= 2 or plan.primary_id.startswith("aberration"):
            counter_val += 0.36 * (_concave_stat(obs.deity_attack) + _concave_stat(obs.deity_health))
        # Economy & scaling counters
        counter_val += 5.5 * _c("base_max_gold_bonus")
        counter_val += 1.3 * _c("bonus_gold_next_turn")

        val += hand_val + counter_val * max(0.35, turn_horizon)

        # 8. Board slot efficiency: avoid empty slots early (T1-T5), allow 1 flex slot if APM/summon needs it
        n_board = len(board)
        expected_min = min(7, obs.turn + 1)
        if n_board < expected_min and obs.turn <= 6:
            val -= 4.5 * (expected_min - n_board)

        return val

    def _tier_and_economy_value(
        self,
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
        in_turn: bool = False,
    ) -> float:
        tier = min(7, max(1, obs.tavern_tier))
        turn = obs.turn
        eff_hp = obs.health + obs.armor

        # Smooth, cliff-free Tavern Tier curve + Anti-Stall Upgrade Discipline + Guiding Candle awareness!
        # Never let Turn 6-9 stall at Tier 3 or Turn 9-11 stall at Tier 4!
        owns_candle = self._candle_id > 0 and any(tid == self._candle_id for tid in obs.trinkets)
        raw_th = (19.0 - turn) / 8.5
        horizon = max(0.55, min(1.25, raw_th))
        if plan.threats.is_finals:
            horizon = 0.12
        hp_factor = 1.08 if (eff_hp >= 26 or plan.threats.next_is_ghost) else (
            0.95 if eff_hp >= 16 else 0.62
        )

        # Discipline against Turn 6-7 suicide-leveling to T5/T6 with an empty board
        board_stats = sum(u.attack + u.health for u in obs.board)
        weak_board_pen = 0.0
        if tier >= 5 and turn <= 7 and board_stats < 55 and not plan.threats.next_is_ghost:
            weak_board_pen = -10.0 * (tier - 4)
        elif tier >= 6 and turn <= 8 and board_stats < 100 and not plan.threats.next_is_ghost:
            weak_board_pen = -12.0

        # Guiding Candle caps shop value at Tier 4
        if owns_candle and tier > 4:
            tier_val = _TIER_BASE_VAL[4] + 0.25 * (_TIER_BASE_VAL[tier] - _TIER_BASE_VAL[4])
        else:
            tier_val = _TIER_BASE_VAL[tier]

        # Standard Curve: Turn 2 upgrade to Tier 2 is almost always optimal!
        if turn == 2 and tier >= 2:
            tier_val += 11.0
        # Target tier alignment
        if tier >= plan.target_tier:
            tier_val += 5.0
        if plan.primary is not None and plan.commitment >= 0.60 and tier > plan.primary.comp.cap_tier:
            tier_val -= 6.0 * (tier - plan.primary.comp.cap_tier)

        val = tier_val * horizon * hp_factor + weak_board_pen

        # Smooth Anti-Stall Penalties (mirrors high-MMR curve discipline so cheap spells NEVER block leveling!)
        n_board = len(obs.board)
        upg_cost = int(obs.upgrade_cost)
        if not owns_candle and not plan.threats.is_finals:
            upg_safety = max(0.0, min(1.0, (eff_hp - 14.0) / 12.0))
            if plan.threats.next_is_ghost:
                upg_safety = 1.0
            bargain_safety = max(0.0, min(1.0, (eff_hp - 9.0) / 8.0))
            if turn >= 4 and tier == 1:
                val -= 8.0 * (turn - 2)
            if upg_cost == 0 and tier < 6 and turn >= 5 and eff_hp >= 5:
                val -= 14.0
            if n_board >= 5:
                if turn >= 5 and tier < 3 and upg_cost <= 5:
                    val -= 11.0 * (3 - tier) * upg_safety
                elif turn >= 6 and tier < 3:
                    val -= 13.0 * (3 - tier) * bargain_safety
                elif turn >= 7 and tier < 4 and upg_cost <= 5:
                    val -= 12.5 * (4 - tier) * upg_safety
                elif turn >= 8 and tier < 4:
                    val -= (15.0 if upg_cost <= 4 else 12.0) * (4 - tier) * bargain_safety
                elif turn >= 9 and tier < 5 and upg_cost <= 6:
                    val -= 12.0 * upg_safety
                elif turn >= 10 and tier < 5:
                    val -= 14.0 * upg_safety
                elif turn >= 11 and tier < 6 and upg_cost <= 6:
                    val -= 10.5 * upg_safety
        # Small credit for accumulated upgrade cost discount toward next tier
        if tier < 6:
            base_c = (0, 5, 7, 8, 10, 10)[tier]
            disc = max(0, min(5, base_c - upg_cost))
            val += 0.55 * disc * horizon * hp_factor

        # During mid-turn beam pruning (`in_turn=True`), remaining gold STILL has real purchasing power (~2.6/g)!
        # At turn end (`in_turn=False`), unspent gold is wasted (only 0.2/g tie-break).
        gold_weight = 2.65 if in_turn else 0.20
        val += gold_weight * float(obs.gold)
        val += 1.25 * float(obs.free_refreshes) + 1.55 * float(obs.bonus_gold_next_turn)

        # Frozen shop value: reward freezing genuine lynchpins/triples/huge shop eat targets, penalize freezing junk!
        if obs.is_frozen and obs.shop:
            best_freeze = 0.0
            for s in obs.shop:
                if s.is_spell:
                    sw = plan.card_desire(s.card_id, s.name, s.tavern_tier, is_spell=True)
                    best_freeze = max(best_freeze, sw * 0.40)
                else:
                    dw = plan.card_desire(s.card_id, s.name, s.tavern_tier, is_spell=False)
                    if plan.owned_non_golden_counts.get(s.card_id, 0) >= 2:
                        dw += 14.0  # Triple frozen in shop!
                    if (s.attack + s.health) >= max(24, 5 * turn):
                        dw += 8.0  # Huge shop minion (Eat-the-Tavern / buffed shop!)
                    best_freeze = max(best_freeze, dw * 0.55)
            # Freezing costs a free natural refresh next turn (~4.0 pts opportunity cost)
            val += best_freeze - 4.0

        return val

    def evaluate_fast(
        self,
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
        in_turn: bool = True,
    ) -> float:
        """Fast (<0.02ms) strategic + tempo value estimate for beam pruning."""
        turn_horizon = max(0.25, min(1.0, (16.0 - obs.turn) / 10.5))
        if plan.threats.is_finals:
            turn_horizon = 0.20
        return (
            self._board_power(obs, plan, turn_horizon)
            + self._tier_and_economy_value(obs, plan, in_turn=in_turn)
            + 0.35 * float(obs.health + obs.armor)
        )

    def evaluate_turn_end(
        self,
        game: seaglass.TavernGame,
        plan: BuildPlan,
        opp_board: Sequence[seaglass.Unit],
        opp_tier: int,
        seed: int = 101,
        rollouts: int = 12,
        ctx: Optional[seaglass.LobbyContext] = None,
        particles: Optional[SovereignOpponentParticles] = None,
    ) -> float:
        """
        Full 1-Turn Combat + `start_turn()` Rust Physics & 1st-Place Evaluation:
          1. Fast comp-position board
          2. Execute `sim.end_turn()` (resolves all EoT triggers in Rust!)
          3. Run paired-seed `equity_against` across opponent particles + Highroll CVaR + Lobby Boss check
          4. Step `resolve_combat_against` + `start_turn()` so ALL permanent combat scaling
             (`Persistent Poet`, `Drustfallen Butcher`, `Lichling Hoarder`, `Turquoise Skitterer`,
             `Ravaging Scorpid`, `Plaguerunner`, `Sanguine Refiner`, `Eudora`, `Eternal Knight`,
             `Accord-o-Tron`, `Overconfidence`, `Lockbox`, `Waveling`) is measured with 100% exact physics!
          5. Fuse post-combat Turn `t+1` build progress, economy, combat ICM, and optional calibrated ValueNet.
        """
        sim = game.copy()
        sim.reseed(seed)
        for _ in range(4):
            o_chk = sim.observe()
            if o_chk.discover_pending is None and not o_chk.trinket_options:
                break
            prompts = [a for a in sim.legal_actions() if a.kind in ("choose_discover", "choose_trinket")]
            if not prompts:
                break
            sim.step(prompts[0])
        obs_pre = sim.observe()
        self.positioner.apply_fast_order(sim, plan)
        if sim.is_legal(seaglass.TavernAction.end_turn()):
            sim.end_turn()
        obs_post = sim.observe()

        # Resolve opponent boards from particle filter (or single board fallback)
        if particles is not None and particles.boards:
            opp_boards = list(particles.boards)
            eff_opp_tier = max(1, int(particles.tier))
            is_ghost = bool(particles.is_ghost or plan.threats.next_is_ghost)
        else:
            opp_boards = [list(opp_board)]
            eff_opp_tier = max(1, int(opp_tier))
            is_ghost = bool(plan.threats.next_is_ghost)

        eff_hp = float(obs_post.health + obs_post.armor)
        damage_cap = ctx.damage_cap if ctx is not None else None
        alive_units = ctx.alive_units if ctx is not None else plan.threats.alive_units

        # ICM-weighted combat importance (1st-place + Finals Counter-Tech calibrated!)
        if plan.threats.is_finals:
            combat_scale = 4.20
        elif alive_units <= 3 and eff_hp <= 18.0:
            combat_scale = 3.10
        elif eff_hp <= 15.0:
            combat_scale = 2.40
        elif eff_hp <= 24.0:
            combat_scale = 1.45
        elif obs_post.turn <= 4:
            combat_scale = 1.15
        else:
            combat_scale = 1.05

        combat_val = 0.0
        lethal_prob = 0.0
        n_opp = max(1, len(opp_boards))

        if is_ghost:
            combat_val = 20.0
        else:
            for oi, ob in enumerate(opp_boards):
                dist = sim.equity_against(ob, eff_opp_tier, rollouts, seed + oi * 37)
                worst_loss = float(max(0, -dist.damage_min))
                if damage_cap is not None:
                    worst_loss = min(worst_loss, float(damage_cap))
                expected_loss = float(max(0.0, -dist.damage_mean))
                if damage_cap is not None:
                    expected_loss = min(expected_loss, float(damage_cap))

                if eff_hp - expected_loss <= 0.0:
                    lethal_prob += dist.b_win_rate
                elif eff_hp - worst_loss <= 0.0:
                    lethal_prob += 0.45 * dist.b_win_rate

                lethal_pen = 0.0
                if eff_hp - expected_loss <= 0.0:
                    lethal_pen = -42.0 * dist.b_win_rate
                elif eff_hp - worst_loss <= 0.0:
                    lethal_pen = -18.0 * dist.b_win_rate

                dealt = max(0.0, dist.damage_mean)
                if damage_cap is not None:
                    dealt = min(dealt, float(damage_cap))
                ko_mult = 1.50 if alive_units <= 4 else 1.0

                combat_val += combat_scale * (
                    34.0 * dist.a_win_rate
                    + 13.0 * dist.draw_rate
                    + 1.25 * (ko_mult * dealt - expected_loss)
                    + lethal_pen
                )
            combat_val /= n_opp

            # Highroll CVaR guard against sudden lethal knockouts!
            if (
                particles is not None
                and particles.highroll_board
                and (damage_cap is None or eff_hp <= float(damage_cap) + 6.0)
            ):
                hr_dist = sim.equity_against(
                    particles.highroll_board,
                    min(6, eff_opp_tier + 1),
                    max(6, rollouts // 2),
                    seed + 3571,
                )
                hr_worst = float(max(0, -hr_dist.damage_min))
                if damage_cap is not None:
                    hr_worst = min(hr_worst, float(damage_cap))
                if eff_hp - hr_worst <= 0.0:
                    combat_val -= 34.0 * hr_dist.b_win_rate

            # 1ST-PLACE OBJECTIVE: explicitly reward beating the Lobby Boss from Turn 8+!
            if (
                obs_post.turn >= 8
                and eff_hp >= 14.0
                and particles is not None
                and particles.lobby_boss_board
                and particles.lobby_boss_player != particles.opponent_player
            ):
                boss_dist = sim.equity_against(
                    particles.lobby_boss_board,
                    max(1, int(particles.lobby_boss_tier)),
                    max(6, rollouts // 2),
                    seed + 4999,
                )
                boss_val = 28.0 * (boss_dist.a_win_rate + 0.45 * boss_dist.draw_rate) + 0.25 * max(
                    0.0, boss_dist.damage_mean
                )
                combat_val += (0.45 if obs_post.turn >= 11 else 0.28) * boss_val

        # Exact Turn `t+1` physics rollout: step `resolve_combat_against` + `start_turn()`!
        # In Top-2 Finals (`is_finals`), winning THIS fight is almost the entire game (`turn_horizon = 0.04`),
        # so the bot freely sells dead scaling units (`Humon'gozz`, `Brann`, `Drakkari`) for Scam/Tech!
        turn_horizon = max(0.24, min(1.0, (16.5 - obs_post.turn) / 10.5))
        if plan.threats.is_finals:
            turn_horizon = 0.04
        elif alive_units <= 3 and eff_hp <= 15.0:
            turn_horizon = 0.10

        next_obs_samples: List[seaglass.TavernObservation] = []
        n_phys = min(2, len(opp_boards))
        for pi in range(n_phys):
            g_next = sim.copy()
            c_seed = (obs_pre.turn * 131071 + pi * 8191 + 3) | 1
            g_next.reseed(c_seed)
            g_next.resolve_combat_against(opp_boards[pi], opponent_tier=eff_opp_tier, seed=c_seed)
            # Decouple Start-of-Turn RNG from combat RNG shifts across candidates!
            g_next.reseed((obs_pre.turn * 524287 + pi * 16381 + 11) | 1)
            g_next.start_turn()
            next_obs_samples.append(g_next.observe())

        if next_obs_samples:
            # Evaluate strategic board & economy at Turn `t+1` (excluding fresh un-frozen random shop noise)
            next_strat = 0.0
            for no in next_obs_samples:
                next_strat += (
                    self._board_power(no, plan, turn_horizon)
                    + self._tier_and_economy_value(no, plan, in_turn=False)
                )
            next_strat /= len(next_obs_samples)
            freeze_adj = (
                self._tier_and_economy_value(obs_post, plan, in_turn=False)
                - self._tier_and_economy_value(next_obs_samples[0], plan, in_turn=False)
            )
            survival_prob = max(0.0, min(1.0, 1.0 - (lethal_prob / n_opp)))
            strat_scale = 0.45 if plan.threats.is_finals else 1.0
            strategic_val = strat_scale * ((0.30 + 0.70 * survival_prob) * next_strat + 0.35 * freeze_adj)
        else:
            strat_scale = 0.45 if plan.threats.is_finals else 1.0
            strategic_val = strat_scale * (
                self._board_power(obs_post, plan, turn_horizon)
                + self._tier_and_economy_value(obs_post, plan, in_turn=False)
            )

        # Properly scaled calibrated ValueNet (`predict_batch` is ALREADY on the `[-100, +100]` scale!)
        neural_bonus = 0.0
        if self.predictor is not None and self.predictor.is_ready and self.neural_weight > 0.0:
            if next_obs_samples:
                net_vals = self.predictor.predict_batch(next_obs_samples, ctx=ctx, end_of_turn=False)
                net_ev = sum(net_vals) / len(net_vals)
            else:
                net_ev = self.predictor.predict_batch([obs_post], ctx=ctx, end_of_turn=True)[0]
            nw = self.neural_weight * (0.40 if plan.threats.is_finals else 1.0)
            neural_bonus = nw * net_ev

        return strategic_val + combat_val + neural_bonus
