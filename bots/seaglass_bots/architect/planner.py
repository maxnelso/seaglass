"""Dynamic Composition Planner (`CompPlanner`) and Build Commitment State (`BuildPlan`).

Each turn, `CompPlanner.analyze(obs, ctx)` evaluates every composition legal in the
current 5-tribe lobby (`ctx.tribes`), measures how close the player is to completing
each comp (owned lynchpins, scalers, multipliers, trinkets, engine counters, and shop
offers), and produces a `BuildPlan` that guides:
  - Card buying, freezing, discovering, and selling protection
  - Leveling vs. rolling tempo conditioned on `primary_comp.home_tier / cap_tier`
  - Missing-piece puzzle-completion bonuses (e.g., `Drustfallen Butcher` when owning
    `Snazzy Phantom`, `Hot-Air Surveyor` when owning `Victorious Geomant`, etc.)
  - Endgame scam / counter-tech pivots against the remaining lobby threats
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict, FrozenSet, List, Optional, Sequence, Tuple

import seaglass

from .comps import CompRegistry, CompSpec, get_comp_registry


@dataclass(frozen=True)
class CompScore:
    comp: CompSpec
    owned_score: float
    shop_pull: float
    total_score: float
    lynchpin_count: int
    scaler_count: int
    multiplier_count: int
    owned_core_ids: FrozenSet[int]


@dataclass(frozen=True)
class ThreatProfile:
    alive_units: int
    next_is_ghost: bool
    next_opp_max_stat: int
    next_opp_total_stats: int
    next_opp_ds_count: int
    next_opp_cleave: bool
    next_opp_reborn_taunt: bool
    next_opp_board: Tuple[seaglass.Unit, ...]
    next_opp_tier: int
    next_opp_seen_age: int
    lobby_max_stat: int
    scam_urgency: float
    is_finals: bool


@dataclass(frozen=True)
class BuildPlan:
    turn: int
    lobby_tribes: FrozenSet[str]
    primary: Optional[CompScore]
    secondary: Optional[CompScore]
    ranked: Tuple[CompScore, ...]
    # 0.0 = pure tempo/open, 0.35..0.65 = leaning/soft commit, 0.70..1.0 = committed
    commitment: float
    target_tier: int
    active_rules: FrozenSet[str]
    threats: ThreatProfile
    # Owned board + hand card_id counts for fast pair/triple and puzzle checks
    owned_counts: Dict[int, int]
    owned_non_golden_counts: Dict[int, int]
    registry: CompRegistry

    @property
    def primary_id(self) -> str:
        return self.primary.comp.comp_id if self.primary is not None else "tempo_open"

    @property
    def primary_name(self) -> str:
        return self.primary.comp.name if self.primary is not None else "Open Tempo"

    def comp_card_weight(self, comp: CompSpec, card_id: int, card_name: str = "") -> Tuple[float, str]:
        """Return `(weight, role)` for a card within `comp`."""
        cid_id = comp.comp_id
        reg = self.registry
        w = reg.lynchpin_ids[cid_id].get(card_id)
        if w is not None:
            return w, "lynchpin"
        w = reg.multiplier_ids[cid_id].get(card_id)
        if w is not None:
            return w, "multiplier"
        w = reg.scaler_ids[cid_id].get(card_id)
        if w is not None:
            return w, "scaler"
        w = reg.support_ids[cid_id].get(card_id)
        if w is not None:
            return w, "support"
        if card_name:
            sw = comp.spells.get(card_name)
            if sw is not None:
                return sw, "spell"
        return 0.0, "none"

    def card_desire(
        self,
        card_id: int,
        card_name: str = "",
        tavern_tier: int = 1,
        is_spell: bool = False,
    ) -> float:
        """
        Compute build-plan marginal desire for acquiring `card_id` (shop buy, discover, freeze).
        Includes puzzle-completion bonus for missing lynchpins/multipliers/scalers!
        """
        cname = card_name or self.registry.id_to_name.get(card_id, "")
        desire = 0.0

        # 1. Primary comp fit (scaled by commitment)
        if self.primary is not None and self.commitment > 0.05:
            comp = self.primary.comp
            w, role = self.comp_card_weight(comp, card_id, cname)
            if w > 0.0:
                owned_n = self.owned_counts.get(card_id, 0)
                # Huge puzzle-completion bonus for the FIRST copy of a missing core piece!
                if owned_n == 0 and role in ("lynchpin", "multiplier", "scaler"):
                    puzzle_mult = 1.45 if role in ("lynchpin", "multiplier") else 1.25
                elif owned_n == 2 and self.owned_non_golden_counts.get(card_id, 0) >= 2:
                    # Completing a triple of a comp card!
                    puzzle_mult = 1.55
                elif owned_n == 1 and role in ("lynchpin", "scaler", "multiplier"):
                    puzzle_mult = 1.15
                else:
                    puzzle_mult = 0.85
                desire += w * puzzle_mult * (0.45 + 0.85 * self.commitment)

        # 2. Secondary comp / pivot fit (keeps pivot outs alive when not hard-locked)
        if self.secondary is not None and self.commitment < 0.85:
            s_comp = self.secondary.comp
            sw, srole = self.comp_card_weight(s_comp, card_id, cname)
            if sw > 0.0:
                pivot_weight = (1.0 - 0.65 * self.commitment) * 0.45
                if srole == "lynchpin" and s_comp.cap_ceiling >= 88.0:
                    pivot_weight *= 1.35
                desire += sw * pivot_weight

        # 3. High-ceiling lynchpin discovery even while Open Tempo (Turns 4-8)
        if self.commitment < 0.45 and self.turn >= 4 and not is_spell:
            best_open = 0.0
            for cs in self.ranked[:6]:
                w, role = self.comp_card_weight(cs.comp, card_id, cname)
                if role in ("lynchpin", "multiplier") and w >= 12.5:
                    cand = w * (cs.comp.cap_ceiling / 90.0) * 0.60
                    if cand > best_open:
                        best_open = cand
            desire = max(desire, best_open)

        # 4. Endgame scam / tech desire conditioned on opponent threat profile
        if self.threats.scam_urgency > 0.15:
            su = self.threats.scam_urgency
            if cname in ("Leeroy the Reckless", "Bile Spitter", "Tide Oracle Morgl"):
                desire += (16.0 if cname == "Tide Oracle Morgl" else 13.0) * su
            elif cname == "Deadly Spore":
                desire += 9.5 * su
            elif cname == "Sin'dorei Straight Shot" and self.threats.next_opp_reborn_taunt:
                desire += 14.0 * su
            elif cname == "Boom-in-a-Box" and self.threats.next_opp_ds_count >= 2:
                desire += 13.5 * su
            elif cname == "Heroic Underdog" and self.threats.next_opp_max_stat >= 60:
                desire += 10.5 * su
            elif cname in ("Upper Hand", "Sharing is Caring"):
                desire += 11.0 * su

        # 5. Universal Tier-7 monsters are almost always worth grabbing
        if tavern_tier >= 7 and not is_spell:
            desire += 16.0

        return desire

    def sell_protection(self, unit: seaglass.Unit) -> float:
        """
        How fiercely should this board minion be protected from selling/cycling?
        Higher = never sell; near 0 = prime flex/cycle sell candidate.
        """
        cid = unit.card_id
        cname = unit.name or self.registry.id_to_name.get(cid, "")
        prot = 0.0

        # Golden high-tier or huge stat carriers are protected
        stat_sum = unit.attack + unit.health
        if unit.is_golden:
            prot += 8.0 + 1.8 * unit.tavern_tier
        if stat_sum >= 40:
            prot += min(28.0, 0.22 * stat_sum)
        if unit.divine_shield and stat_sum >= 24:
            prot += 6.0
        if cid in self.registry.cleave_ids and stat_sum >= 20:
            prot += 10.0

        # Spent one-shot Battlecry minions have ZERO ongoing engine protection on board
        # (unless Young Murk-Eye or Kelp Keeper is actively re-triggering them!)
        has_bc_retrigger = (
            self.owned_counts.get(self.registry.name_to_id.get("Young Murk-Eye", -1), 0) > 0
            or self.owned_counts.get(self.registry.name_to_id.get("Kelp Keeper", -1), 0) > 0
        )
        spent_bc_names = (
            "Captain Sanders",
            "Elite Navigator",
            "Silent Deliverer",
            "Primalfin Lookout",
            "Rodeo Performer",
            "Imposing Percussionist",
            "Tavern Tempest",
            "Auto Accelerator",
            "Maw Caster",
            "Disguised Graverobber",
            "Fetid Corroder",
            "Joyous",
            "Razorfen Geomancer",
            "Nerubian Deathswarmer",
            "Azsharan Cutlassier",
            "Refreshing Anomaly",
            "En-Djinn Blazer",
            "Firelands Fugitive",
            "Lovesick Balladist",
            "Ominous Seer",
            "Mind Muck",
            "Bubble Gunner",
            "Southsea Busker",
            "Tad",
        )
        is_spent_bc = (cname in spent_bc_names) and not has_bc_retrigger

        if self.primary is not None and self.commitment > 0.15 and not is_spent_bc:
            w, role = self.comp_card_weight(self.primary.comp, cid, cname)
            if role == "lynchpin":
                prot += (18.0 + 1.1 * w) * (0.5 + 0.7 * self.commitment)
            elif role == "multiplier":
                prot += (16.0 + 1.0 * w) * (0.5 + 0.7 * self.commitment)
            elif role == "scaler":
                prot += (9.0 + 0.7 * w) * (0.4 + 0.6 * self.commitment)
            elif role == "support":
                prot += 2.5 * self.commitment
            elif self.commitment >= 0.55 and stat_sum < 140:
                # Off-comp unit on a committed board: make it easy to replace with real comp pieces!
                prot *= 0.65

        # Decisive Finals / Top-3 combat turn: pure Tavern-only scaling units (Brann, Drakkari, Kalecgos,
        # Humon'gozz, Rogers, Rewinder, Felfire, Drone Duplicator, etc.) lose protection so the bot
        # freely swaps them for Leeroy / Bile Spitter / Deadly Spore / Sin'dorei / Boom-in-a-Box!
        if (self.threats.is_finals or (self.threats.alive_units <= 3 and self.threats.scam_urgency >= 0.55)) and self.turn >= 11:
            if cname in (
                "Brann Bronzebeard",
                "Drakkari Enchanter",
                "Soul Rewinder",
                "Ashen Corruptor",
                "Kalecgos, Arcane Aspect",
                "Sky Admiral Rogers",
                "Proud Privateer",
                "Living Azerite",
                "Felfire Conjurer",
                "Cataclysmic Harbinger",
                "Humon'gozz",
                "Gem Rat",
                "Auto Accelerator",
                "Drone Duplicator",
                "Mindbending Recruiter",
                "Brain Rotter",
                "Abyssal Envoy",
                "Amber Guardian",
                "Treasure Parrot",
                "Kelp Keeper",
            ) and stat_sum < 180:
                prot *= 0.12

        # Protect scam units when scam_urgency is high
        if self.threats.scam_urgency >= 0.30 and (
            unit.venomous or cid in self.registry.scam_ids
        ):
            prot += 22.0 * self.threats.scam_urgency

        return prot


class CompPlanner:
    """Evaluates lobby tribes, board/hand/trinket/counter state, and opponent threats to build a `BuildPlan`."""

    def __init__(self, catalog_name: str = "full") -> None:
        self.catalog_name = catalog_name
        self.registry: CompRegistry = get_comp_registry(catalog_name)
        self._trinket_id_to_name: Dict[int, str] = {
            tid: tname for tid, tname, _g, _c, _impl in seaglass.trinkets_table()
        }

    def analyze_threats(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext],
    ) -> ThreatProfile:
        if ctx is None:
            return ThreatProfile(
                alive_units=8,
                next_is_ghost=False,
                next_opp_max_stat=0,
                next_opp_total_stats=0,
                next_opp_ds_count=0,
                next_opp_cleave=False,
                next_opp_reborn_taunt=False,
                next_opp_board=(),
                next_opp_tier=obs.tavern_tier,
                next_opp_seen_age=99,
                lobby_max_stat=0,
                scam_urgency=0.0,
                is_finals=False,
            )

        alive_units = int(ctx.alive_units)
        next_is_ghost = bool(ctx.next_is_ghost)
        next_board: Tuple[seaglass.Unit, ...] = ()
        next_tier = obs.tavern_tier
        next_seen_age = 99
        next_max_stat = 0
        next_total_stats = 0
        next_ds = 0
        next_cleave = False
        next_reborn_taunt = False
        lobby_max_stat = 0

        for opp in ctx.opponents:
            if not opp.alive:
                continue
            sb = list(opp.seen_board)
            for u in sb:
                mx = max(u.attack, u.health)
                if mx > lobby_max_stat:
                    lobby_max_stat = mx
            if opp.is_next:
                next_board = tuple(sb)
                next_tier = int(opp.tier)
                next_seen_age = (
                    obs.turn - int(opp.seen_turn) if opp.seen_turn is not None else 99
                )
                for u in sb:
                    next_max_stat = max(next_max_stat, u.attack, u.health)
                    next_total_stats += u.attack + u.health
                    if u.divine_shield:
                        next_ds += 1
                    if u.card_id in self.registry.cleave_ids:
                        next_cleave = True
                    if u.reborn or u.taunt:
                        next_reborn_taunt = True

        my_total_stats = sum(u.attack + u.health for u in obs.board)
        my_max_stat = max((max(u.attack, u.health) for u in obs.board), default=0)
        eff_hp = obs.health + obs.armor
        is_finals = alive_units <= 2 and not next_is_ghost

        scam_urgency = 0.0
        if obs.turn >= 9 and not next_is_ghost:
            if is_finals:
                scam_urgency += 0.45
            elif alive_units <= 4:
                scam_urgency += 0.22
            if eff_hp <= 15:
                scam_urgency += 0.25
            # Enemy has massive stats compared to ours
            if next_max_stat >= max(65, int(1.6 * max(1, my_max_stat))):
                scam_urgency += 0.40
            elif lobby_max_stat >= max(90, int(2.0 * max(1, my_max_stat))):
                scam_urgency += 0.25
            if next_total_stats >= max(180, int(1.45 * max(1, my_total_stats))):
                scam_urgency += 0.25
        scam_urgency = min(1.0, scam_urgency)

        return ThreatProfile(
            alive_units=alive_units,
            next_is_ghost=next_is_ghost,
            next_opp_max_stat=next_max_stat,
            next_opp_total_stats=next_total_stats,
            next_opp_ds_count=next_ds,
            next_opp_cleave=next_cleave,
            next_opp_reborn_taunt=next_reborn_taunt,
            next_opp_board=next_board,
            next_opp_tier=next_tier,
            next_opp_seen_age=next_seen_age,
            lobby_max_stat=lobby_max_stat,
            scam_urgency=scam_urgency,
            is_finals=is_finals,
        )

    def analyze(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> BuildPlan:
        reg = self.registry
        lobby_tribes: FrozenSet[str] = (
            frozenset(str(t).lower() for t in ctx.tribes)
            if ctx is not None and ctx.tribes
            else frozenset(
                {
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
                }
            )
        )

        threats = self.analyze_threats(obs, ctx)

        owned_units = [u for u in obs.board] + [c for c in obs.hand if not c.is_spell]
        owned_counts: Dict[int, int] = {}
        owned_non_golden: Dict[int, int] = {}
        for u in owned_units:
            owned_counts[u.card_id] = owned_counts.get(u.card_id, 0) + 1
            if not u.is_golden:
                owned_non_golden[u.card_id] = owned_non_golden.get(u.card_id, 0) + 1

        owned_trinket_names = [
            self._trinket_id_to_name.get(tid, "") for tid in obs.trinkets
        ]
        counters = obs.player_counters

        scored: List[CompScore] = []
        for comp in reg.comps:
            if any(req not in lobby_tribes for req in comp.required_tribes):
                continue

            cid = comp.comp_id
            l_map = reg.lynchpin_ids[cid]
            s_map = reg.scaler_ids[cid]
            m_map = reg.multiplier_ids[cid]
            sup_map = reg.support_ids[cid]

            owned_score = 0.0
            lynchpin_count = 0
            scaler_count = 0
            multiplier_count = 0
            core_ids = set()
            seen_per_card: Dict[int, int] = {}

            for u in owned_units:
                ucid = u.card_id
                occ = seen_per_card.get(ucid, 0)
                seen_per_card[ucid] = occ + 1
                dim = 1.0 if occ == 0 else (0.55 if occ == 1 else 0.30)
                g_mult = 1.45 if u.is_golden else 1.0

                if ucid in l_map:
                    owned_score += l_map[ucid] * g_mult * dim
                    lynchpin_count += 1
                    core_ids.add(ucid)
                elif ucid in m_map:
                    owned_score += m_map[ucid] * g_mult * dim * 0.90
                    multiplier_count += 1
                    core_ids.add(ucid)
                elif ucid in s_map:
                    owned_score += s_map[ucid] * g_mult * dim * 0.80
                    scaler_count += 1
                    core_ids.add(ucid)
                elif ucid in sup_map:
                    owned_score += sup_map[ucid] * g_mult * dim * 0.42

            # Synergy coherence multiplier: owning pieces from multiple buckets means the comp is actually online!
            if lynchpin_count >= 1 and (scaler_count >= 1 or multiplier_count >= 1):
                owned_score *= 1.28
            elif lynchpin_count >= 2:
                owned_score *= 1.22
            elif lynchpin_count == 0 and multiplier_count >= 1 and scaler_count == 0:
                # Alone, a generic multiplier (Brann/Titus/Drakkari) doesn't commit to one comp
                owned_score *= 0.55

            # Owned Trinkets are a huge direction signal
            for tname in owned_trinket_names:
                tw = comp.trinkets.get(tname)
                if tw:
                    owned_score += tw * 1.15
                    lynchpin_count += 1

            # Persistent engine counters (Undead Atk, Blood Gems, Discards, Magnetizations, Spell Power, Shop Buffs)
            counter_pts = 0.0
            for c_idx, scale in reg.counter_idx_signals[cid]:
                if c_idx < len(counters):
                    val = float(counters[c_idx])
                    if val > 0.0:
                        counter_pts += min(24.0, val * scale)
            owned_score += counter_pts

            # Bonus tribe lobby affinity
            bonus_matches = sum(1 for bt in comp.bonus_tribes if bt in lobby_tribes)
            lobby_mult = 1.0 + 0.04 * bonus_matches

            # Shop pull: if a Tier 4+ lynchpin is sitting in the current shop, surface that direction!
            shop_pull = 0.0
            for s in obs.shop:
                if s.is_spell:
                    sw = comp.spells.get(s.name, 0.0)
                    if sw >= 10.0 and owned_score >= 12.0:
                        shop_pull += sw * 0.25
                else:
                    lw = l_map.get(s.card_id, 0.0)
                    if lw >= 11.5:
                        shop_pull += lw * 0.55
                    elif s.card_id in m_map and owned_score >= 10.0:
                        shop_pull += m_map[s.card_id] * 0.40

            # Weight total score slightly by comp's late-game cap ceiling
            cap_factor = 0.85 + 0.20 * (comp.cap_ceiling / 100.0)
            total_score = (owned_score + 0.65 * shop_pull) * lobby_mult * cap_factor

            scored.append(
                CompScore(
                    comp=comp,
                    owned_score=owned_score,
                    shop_pull=shop_pull,
                    total_score=total_score,
                    lynchpin_count=lynchpin_count,
                    scaler_count=scaler_count,
                    multiplier_count=multiplier_count,
                    owned_core_ids=frozenset(core_ids),
                )
            )

        scored.sort(key=lambda x: (x.total_score, x.owned_score, x.comp.cap_ceiling), reverse=True)

        primary = scored[0] if scored else None
        secondary = scored[1] if len(scored) > 1 else None

        # Compute commitment strictly from real build-around ownership (NEVER lock from T1/T2 support alone!)
        commitment = 0.0
        if primary is not None:
            oscore = primary.owned_score
            l_cnt = primary.lynchpin_count
            sm_cnt = primary.scaler_count + primary.multiplier_count

            if obs.turn <= 3:
                # Turns 1-3: stay open tempo
                commitment = min(0.15, oscore / 100.0)
            elif obs.turn == 4:
                commitment = min(0.32, max(0.0, (oscore - 10.0) / 55.0)) if l_cnt >= 1 else min(0.18, oscore / 90.0)
            elif obs.turn <= 6:
                if l_cnt >= 2 or (l_cnt >= 1 and sm_cnt >= 2):
                    commitment = min(0.72, 0.32 + oscore / 65.0)
                elif l_cnt >= 1:
                    commitment = min(0.52, 0.20 + oscore / 75.0)
                else:
                    commitment = min(0.28, oscore / 85.0)
            else:
                if l_cnt >= 2 and sm_cnt >= 1:
                    commitment = min(0.95, 0.55 + oscore / 75.0)
                elif l_cnt >= 2 or (l_cnt >= 1 and sm_cnt >= 2):
                    commitment = min(0.82, 0.42 + oscore / 75.0)
                elif l_cnt >= 1:
                    commitment = min(0.62, 0.28 + oscore / 80.0)
                elif sm_cnt >= 3 and oscore >= 28.0:
                    commitment = min(0.50, 0.20 + oscore / 90.0)
                else:
                    commitment = min(0.28, oscore / 95.0)

        # Target tier for leveling/rolling guidance
        if primary is not None and commitment >= 0.50:
            if obs.turn <= 8 and obs.tavern_tier < primary.comp.home_tier:
                target_tier = primary.comp.home_tier
            elif obs.tavern_tier >= primary.comp.home_tier and (
                obs.turn >= 10 or (primary.owned_score >= 38.0 and obs.health + obs.armor >= 22)
            ):
                target_tier = primary.comp.cap_tier
            else:
                target_tier = primary.comp.home_tier
        else:
            # Open tempo: Standard curve -> T2 on T2, T3 on T5, T4 on T6-7, T5 on T8-9
            if obs.turn <= 2:
                target_tier = 2
            elif obs.turn <= 4:
                target_tier = 3 if obs.turn == 4 else 2
            elif obs.turn <= 6:
                target_tier = 4 if obs.turn == 6 else 3
            elif obs.turn <= 8:
                target_tier = 5 if obs.turn == 8 else 4
            else:
                target_tier = 6 if obs.turn >= 10 else 5

        active_rules: FrozenSet[str] = (
            primary.comp.rules if (primary is not None and commitment >= 0.30) else frozenset()
        )

        return BuildPlan(
            turn=obs.turn,
            lobby_tribes=lobby_tribes,
            primary=primary,
            secondary=secondary,
            ranked=tuple(scored[:6]),
            commitment=commitment,
            target_tier=target_tier,
            active_rules=active_rules,
            threats=threats,
            owned_counts=owned_counts,
            owned_non_golden_counts=owned_non_golden,
            registry=reg,
        )
