"""ArchitectBot v7+: Composition-Driven Superhuman Hybrid Controller.

Combines:
  1. Complete Patch 37.0.2 tactical foundation (multi-particle opponent belief tracking,
     54-counter valuation, exact `t+1` Start-of-Turn Bellman shop pricing, 2-turn CRN
     Trinket rollouts, full Hero Power & Discover continuation, Successive Halving positioning).
  2. First-class 26-Comp Planner (`CompPlanner`) with dynamic commitment, pairwise exponential
     engine completion bonuses, missing-piece puzzle boosts, spent-Battlecry cycling, and
     duplicate non-stacking aura penalties.
  3. Comp-Specific Tavern Sequencing Macros injected directly into beam search:
       - Undead Tavern Reborn (`Snazzy Phantom` right-most carry prep before `Dead Bellringer` / `Butchering`)
       - `Drone Duplicator` Activate-before-Magnetize & never magnetize directly onto `Polarizing Beatboxer`
       - `Victorious Geomant` left-most `Roogug`/`Gatekeeper Amalgam` prep on overflow
       - `Lurking Lionfish` / `Snarky Shark` Rally-leftmost Tavern attack prep
       - `Greedy Conniver` golden (`Elite Navigator` / `Eyes of the Earth Mother`) -> Sell for Tier 7
       - `Arcane Behemoth` Elemental-feeder cycling
       - `Living Prison` Activate-before-Buy biggest shop minion
       - `Cataclysmic Harbinger` filler-before-payoff spell ordering
       - Top-3 / Finals dead-scaler -> Scam (`Leeroy` / `Bile Spitter` / `Deadly Spore` / `Sin'dorei` / `Boom-in-a-Box`) swap
  4. Comp-Aware Positioning Invariants (`Persistent Poet` interior Dragon sandwich, `Snazzy Phantom`
     right-most Undead, `Deathstrider` left-most Deathrattle, `Sanguine Refiner` first, `Y'Shaarj` vs `C'Thun`).
  5. 1st-Place Objective + Ruthless Finals 1v1 Combat Override + Swappable GPU-trained `ArchitectValueNet`.
"""

from __future__ import annotations

from dataclasses import dataclass
import os
from typing import Dict, FrozenSet, List, Optional, Sequence, Tuple

import numpy as np
import seaglass

from ..sovereign_belief import SovereignOpponentParticles
from ..sovereign_evaluator import SovereignEvaluator
from ..sovereign_knowledge import SovereignKnowledge, get_sovereign_knowledge
from ..sovereign_model import SovereignNumpyPredictor
from ..sovereign_positioning import SovereignPositioner
from ..sovereign_search import (
    QueuedStep,
    SovereignBot,
    _action_key,
    _canonical_state_key,
    _decision_skeleton,
    _plan_validation_key,
    expendable_slots,
    prune_sovereign_actions,
)
from .comps import CompRegistry, get_comp_registry
from .evaluator import _HARBINGER_PAYOFF_SPELLS
from .planner import BuildPlan, CompPlanner

DEFAULT_ARCHITECT_NPZ_PATH = os.path.join(
    os.path.dirname(os.path.dirname(__file__)), "data", "architect_value_v1.npz"
)
# 1st-place-sharp placement utility weights (emphasizes winning 1st over settling for 3rd/4th!)
ARCHITECT_PLACE_WEIGHTS = np.array(
    [45.0, 18.0, 6.0, -2.0, -10.0, -18.0, -26.0, -34.0], dtype=np.float32
)

# Empirical-Bayes Patch 37.0.2 Trinket adjustments (calibrated on 1,920 seats / 3,771 Patch 37.0.2 picks!)
ARCHITECT_P37_TRINKET_DELTAS: Dict[str, float] = {
    # Proven Patch 37.0.2 1st-place winners SovereignBot undervalues
    "Yogg-Tastic Pastry": 16.0,
    "Wax Lance": 14.0,          # 30.9% 1st across n=188 picks!
    "Inductive Gyroblade": 14.0,
    "Sky Golem Portrait": 12.0, # 26.8% 1st, avg=2.91 across n=56!
    "Sinister Invitation": 10.0,
    "Aggem Sticker": 10.0,      # 31.8% 1st, avg=3.45 across n=22!
    "Jewelry Box": 9.0,         # 41.7% 1st across n=12!
    "Nomi Sticker": 9.0,        # 33.3% 1st across n=12!
    "Azeroth Model Globe": 8.0,
    "Hackerfin Portrait": 8.0,  # 23.1% 1st, avg=2.69 across n=13!
    "Booty Bay Brew": 7.0,      # 23.1% 1st, avg=3.00 across n=26!
    "Ornate Clock": 6.0,
    "Curator Sticker": 6.0,
    "Portable Factory": 5.5,    # 20.8% 1st across n=24!
    "Tableware Portrait": 4.0,
    "Miniature Ship": 4.5,
    "Book of Medivh": 5.0,
    # Proven Patch 37.0.2 traps (0.0% 1st-place rate across 1,920 seats!)
    "Ghastly Sticker": -24.0,
    "Rendle Sticker": -18.0,        # 0/40 (0.0% 1st, avg=5.45)
    "Warband Whistle": -16.0,       # 0/23 (0.0% 1st, avg=5.87)
    "Stuffed Coin Purse": -16.0,    # 0/19 (0.0% 1st, avg=6.00)
    "Token of the Old Gods": -15.0, # 0/17 (0.0% 1st, avg=5.88)
    "Safety Patch": -14.0,          # 0/16 (0.0% 1st, avg=5.75)
    "Lovely Locket": -14.0,         # 0/17 (0.0% 1st, avg=5.65)
    "Shrine of Evolution": -14.0,   # 0/12 (0.0% 1st, avg=5.92)
    "Gold Mallet": -14.0,
    "Honeycomb Ring": -12.0,        # 0/17 (0.0% 1st, avg=5.24)
    "Sinstone Sticker": -12.0,
    "Manipulator Portrait": -12.0,
    "Volumizer Portrait": -10.0,
    "Innkeeper's Hearth": -10.0,
    "Dalaran Cheese Wheel": -10.0,  # 0/13 (0.0% 1st, avg=5.38)
    "Nether Pendant": -10.0,        # 0/12 (0.0% 1st, avg=5.42)
    "Explorer's Binoculars": -8.0,
    "Reinvigorating Light": -8.0,
    "Magician's Top Hat": -7.5,
    "Guiding Candle": -6.0,
    "Boom's Monster Portrait": -5.0,
}

# Empirical-Bayes Patch 37.0.2 Hero adjustments (restored v10 core + confirmed 768-seat extremes)
ARCHITECT_P37_HERO_DELTAS: Dict[str, float] = {
    "Trade Prince Gallywix": 7.5,
    "Scabbs Cutterbutter": 6.0,
    "Enhance-o Mechano": 5.5,
    "Tess Greymane": 5.0,
    "Tae'thelan Bloodwatcher": 5.0,
    "C'Thun": 4.0,
    "Rokara": 3.5,
    "Mutanus the Devourer": 3.0,
    "Patchwerk": 2.5,
    # Overrated in SovereignBot's stale Patch 36.6 priors
    "Greybough": -7.0,
    "Deathwing": -6.5,
    "Tras'tath, Soul Parasite": -6.5,
    "Rock Master Voone": -6.0,
    "Varden Dawngrasp": -5.5,
    "Thorim, Stormlord": -5.0,
    "Cap'n Hoggarr": -4.5,
    "Vol'jin": -4.5,
    "Tavish Stormpike": -4.0,
    "Chenvaala": -3.5,
    "The Rat King": -3.5,
}


@dataclass(frozen=True)
class ArchitectKnowledge(SovereignKnowledge):
    """Patch 37.0.2 Empirical-Bayes calibrated knowledge base overriding stale Sovereign priors."""

    def hero_lobby_score(self, hero_id: int, lobby_tribes: Sequence[str]) -> float:
        score = super().hero_lobby_score(hero_id, lobby_tribes)
        hname = self.base.hero_names.get(hero_id, "")
        return score + ARCHITECT_P37_HERO_DELTAS.get(hname, 0.0)

    def trinket_prior_score(self, trinket_name: str) -> float:
        return super().trinket_prior_score(trinket_name) + ARCHITECT_P37_TRINKET_DELTAS.get(
            trinket_name, 0.0
        )


def get_architect_knowledge(catalog_name: str = "full") -> ArchitectKnowledge:
    sk = get_sovereign_knowledge(catalog_name)
    return ArchitectKnowledge(base=sk.base, catalog_name=catalog_name)


def prune_architect_actions(
    obs: seaglass.TavernObservation,
    legal: Sequence[seaglass.TavernAction],
    evaluator: "ArchitectHybridEvaluator",
    kb,
    particles: Optional[SovereignOpponentParticles] = None,
    ctx: Optional[seaglass.LobbyContext] = None,
) -> List[seaglass.TavernAction]:
    """
    Zero-bloat wrapper over `prune_sovereign_actions` unlocking ONLY the two strictly positive-EV plays:
      1. Ghost free-leveling (`ctx.next_is_ghost == True`)
      2. Top-2 Finals selling of dead Tavern scalers when holding scam/big upgrades or rolling
    """
    out = prune_sovereign_actions(obs, legal, evaluator, kb, particles=particles, ctx=ctx)
    next_is_ghost = bool(ctx.next_is_ghost) if ctx is not None else False
    alive_units = ctx.alive_units if ctx is not None else 8
    is_finals = alive_units <= 2 and not next_is_ghost
    if not next_is_ghost and not (is_finals and obs.turn >= 10 and len(obs.board) >= 7):
        return out

    seen = {_action_key(a) for a in out}
    hand_has_scam_or_big = any(
        not c.is_spell
        and not c.unplayable
        and (c.venomous or c.card_id in evaluator.reg.scam_ids or (c.attack + c.health) >= 120)
        for c in obs.hand
    )
    for act in legal:
        ak = _action_key(act)
        if ak in seen:
            continue
        k = act.kind
        if k == "upgrade" and next_is_ghost:
            out.append(act)
            seen.add(ak)
        elif k == "sell" and is_finals and obs.turn >= 10 and len(obs.board) >= 7:
            b_pos, _ = act.args
            if b_pos is not None and b_pos < len(obs.board):
                u_sell = obs.board[b_pos]
                if (
                    u_sell.card_id in evaluator._finals_dead_scaler_ids
                    and (u_sell.attack + u_sell.health) < 160
                    and (hand_has_scam_or_big or obs.gold + 1 >= obs.refresh_cost + obs.minion_cost)
                ):
                    out.append(act)
                    seen.add(ak)
    return out


class ArchitectNumpyPredictor(SovereignNumpyPredictor):
    """Pure-NumPy BLAS Set-Transformer predictor using 1st-place-sharp placement weights."""

    def forward_arrays(
        self,
        vocab: np.ndarray,
        slot: np.ndarray,
        pos: np.ndarray,
        num: np.ndarray,
        mask: np.ndarray,
        glob: np.ndarray,
    ) -> np.ndarray:
        # Reuse parent's exact forward pass up to logits by temporarily overriding placement weights
        import seaglass_bots.sovereign_model as sm

        orig = sm.SOV_PLACEMENT_WEIGHTS
        sm.SOV_PLACEMENT_WEIGHTS = ARCHITECT_PLACE_WEIGHTS
        try:
            return super().forward_arrays(vocab, slot, pos, num, mask, glob)
        finally:
            sm.SOV_PLACEMENT_WEIGHTS = orig


class ArchitectHybridPositioner(SovereignPositioner):
    """Extends `SovereignPositioner` with Patch 37.0.2 Comp-Specific Positioning Invariants."""

    def __init__(
        self,
        catalog_name: str = "full",
        max_candidates: int = 68,
        evaluator: Optional[SovereignEvaluator] = None,
    ) -> None:
        super().__init__(
            catalog_name=catalog_name,
            max_candidates=max_candidates,
            evaluator=evaluator,
        )
        self.reg: CompRegistry = get_comp_registry(catalog_name)

        def _id(name: str) -> int:
            return self.reg.name_to_id.get(name, -1)

        self._snazzy_id = _id("Snazzy Phantom")
        self._banshee_id = _id("Barrier Banshee")
        self._deathstrider_id = _id("Deathstrider")
        self._goldrinn_id = _id("Goldrinn, the Great Wolf")
        self._skitterer_id = _id("Turquoise Skitterer")
        self._refiner_id = _id("Sanguine Refiner")
        self._bonker_id = _id("Bonker")
        self._sanguine_champ_id = _id("Sanguine Champion")
        self._broodmother_id = _id("Heroic Broodmother")
        self._vindicator_id = _id("Crimson Vindicator")
        self._tarecgosa_id = _id("Tarecgosa")
        self._poet_id = _id("Persistent Poet")
        self._eternal_summoner_id = _id("Eternal Summoner")
        self._converter_id = _id("Faceless Converter")
        self._drifting_sac_id = _id("Drifting Sacrifice")
        self._underrot_id = _id("Underrot Spawn")
        self._bile_spitter_id = _id("Bile Spitter")

        tag_names, tag_table = seaglass.card_tags()
        t_idx = {n: i for i, n in enumerate(tag_names)}
        dr_col = t_idx.get("deathrattle")
        self._dr_ids: FrozenSet[int] = (
            frozenset(cid for cid, row in tag_table.items() if row[dr_col] > 0.5)
            if dr_col is not None
            else frozenset()
        )
        self._cur_board_ref: Optional[Sequence[seaglass.Unit]] = None
        self._cur_deity: str = ""

    def _comp_invariant_bonus(self, board: Sequence[seaglass.Unit], perm: Sequence[int]) -> float:
        n = len(perm)
        if n <= 1:
            return 0.0
        ordered = [board[i] for i in perm]
        bonus = 0.0

        has_poet = False
        has_snazzy = False
        has_deathstrider = False
        for u in ordered:
            cid = u.card_id
            if cid == self._poet_id:
                has_poet = True
            elif cid == self._snazzy_id:
                has_snazzy = True
            elif cid == self._deathstrider_id:
                has_deathstrider = True

        # 1. Persistent Poet interior Dragon sandwich (NEVER edge slot when >= 2 other scaling Dragons exist!)
        if has_poet:
            other_dragons = sum(
                1
                for u in ordered
                if u.card_id != self._poet_id
                and u.card_id != self._tarecgosa_id
                and ("dragon" in u.tribes or "all" in u.tribes)
            )
            for pos, u in enumerate(ordered):
                if u.card_id == self._poet_id:
                    adj_good = 0
                    for nb_pos in (pos - 1, pos + 1):
                        if 0 <= nb_pos < n:
                            nb = ordered[nb_pos]
                            if ("dragon" in nb.tribes or "all" in nb.tribes) and nb.card_id not in (
                                self._poet_id,
                                self._tarecgosa_id,
                            ):
                                adj_good += 1
                                if nb.card_id in (self._broodmother_id, self._vindicator_id):
                                    bonus += 14.0
                    if other_dragons >= 2:
                        if adj_good == 2:
                            bonus += 18.0
                        elif pos in (0, n - 1):
                            bonus -= 16.0

        # 2. Snazzy Phantom -> right-most Undead carry + Reborn Undead left
        if has_snazzy:
            rm_undead = None
            for idx in range(n - 1, -1, -1):
                utr = ordered[idx].tribes
                if "undead" in utr or "all" in utr:
                    rm_undead = idx
                    break
            if rm_undead is not None:
                carry = ordered[rm_undead]
                if carry.card_id == self._banshee_id or carry.divine_shield:
                    bonus += 18.0
                elif carry.attack + carry.health >= 40:
                    bonus += 10.0
                for pos, u in enumerate(ordered):
                    if u.reborn and pos < rm_undead:
                        bonus += 4.5

        # 3. Deathstrider -> left-most Deathrattle MUST be Goldrinn / Turquoise Skitterer!
        if has_deathstrider:
            for u in ordered:
                if u.card_id in self._dr_ids or u.extra_deathrattles > 0:
                    if u.card_id in (self._goldrinn_id, self._skitterer_id):
                        bonus += 22.0
                    elif u.card_id in (self._eternal_summoner_id, self._sanguine_champ_id):
                        bonus += 12.0
                    else:
                        bonus -= 8.0
                    break

        # 4. Sanguine Refiner attacks FIRST before Bonker; Bile Spitter attacks early!
        for pos, u in enumerate(ordered):
            frac = pos / max(1, n - 1)
            if u.card_id == self._refiner_id:
                bonus += 14.0 * (1.0 - frac)
            elif u.card_id == self._bile_spitter_id:
                bonus += 10.0 * (1.0 - frac)
            elif u.card_id == self._sanguine_champ_id:
                bonus += 10.0 * (1.0 - frac)

        # 5. Aberration Deity death ordering (`Y'Shaarj` vs `C'Thun`)
        if self._cur_deity == "Y'Shaarj":
            for pos, u in enumerate(ordered):
                if "aberration" in u.tribes:
                    frac = pos / max(1, n - 1)
                    bonus += min(12.0, 0.06 * (u.attack + u.health)) * (1.0 - frac)
        elif self._cur_deity == "C'Thun":
            for pos, u in enumerate(ordered):
                if "aberration" in u.tribes:
                    frac = pos / max(1, n - 1)
                    if u.card_id in (self._drifting_sac_id, self._underrot_id, self._converter_id) or u.reborn:
                        bonus += 9.0 * (1.0 - frac)
                    elif u.attack + u.health >= 30:
                        bonus += 6.0 * frac

        return bonus

    def fast_best_perm(
        self,
        board: Sequence[seaglass.Unit],
        obs: Optional[seaglass.TavernObservation] = None,
        particles: Optional[SovereignOpponentParticles] = None,
    ) -> List[int]:
        self._cur_board_ref = board
        self._cur_deity = str(obs.deity or "") if obs is not None else ""
        try:
            return super().fast_best_perm(board, obs=obs, particles=particles)
        finally:
            self._cur_board_ref = None

    def _score_perm_from_feats(
        self,
        perm: Sequence[int],
        feats: List[Tuple],
        is_alakir: bool,
        is_illidan: bool,
        enemy_ds: bool,
        enemy_giant: bool,
        enemy_cleave: bool,
    ) -> float:
        base = super()._score_perm_from_feats(
            perm, feats, is_alakir, is_illidan, enemy_ds, enemy_giant, enemy_cleave
        )
        if self._cur_board_ref is not None and len(self._cur_board_ref) == len(perm):
            base += self._comp_invariant_bonus(self._cur_board_ref, perm)
        return base

    def best_permutation(
        self,
        tavern: seaglass.TavernGame,
        particles: SovereignOpponentParticles,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> Tuple[int, ...]:
        obs = tavern.observe()
        self._cur_board_ref = obs.board
        self._cur_deity = str(obs.deity or "")
        orig_max = self.max_candidates
        # Expand positioning search in Top-3 / Finals where 1 combat decides 1st place!
        if ctx is not None and ctx.alive_units <= 3:
            self.max_candidates = max(orig_max, 96)
        try:
            return super().best_permutation(tavern, particles, ctx=ctx)
        finally:
            self.max_candidates = orig_max
            self._cur_board_ref = None


class ArchitectHybridEvaluator(SovereignEvaluator):
    """
    Wraps `SovereignEvaluator` with:
      1. 26-Comp `CompPlanner` & Pairwise Exponential Engine Completion Bonuses
      2. 1st-Place Objective Calibration (Lobby Boss beat bonus + Exponential Cap boost)
      3. Top-2 / Top-3 Finals Combat Override & Dead-Scaler Unlock for Scam/Tech
      4. Spent One-Shot Battlecry Cycling & Non-Stacking Duplicate Aura Penalties
      5. Swappable Patch 37.0.2 `ArchitectValueNet` (`architect_value_v1.npz` if present, else `sovereign_value_v1.npz`)
    """

    def __init__(
        self,
        catalog_name: str = "full",
        use_physics_rollout: bool = True,
        use_neural_value: bool = True,
        neural_weight: float = 0.24,
        weights_path: Optional[str] = None,
        comp_weight: float = 0.12,
        zero_net_weight: float = 0.0,
        zero_weights_path: Optional[str] = None,
        dd_hold_weight: float = 0.0,
        fast_curve_weight: float = 0.0,
    ) -> None:
        # Automatically load `architect_value_v1.npz` if trained and no explicit path given!
        eff_weights = weights_path
        if eff_weights is None and os.path.exists(DEFAULT_ARCHITECT_NPZ_PATH):
            eff_weights = DEFAULT_ARCHITECT_NPZ_PATH

        super().__init__(
            catalog_name=catalog_name,
            use_physics_rollout=use_physics_rollout,
            use_neural_value=use_neural_value,
            neural_weight=neural_weight,
            weights_path=eff_weights,
        )
        # Plug in Patch 37.0.2 Empirical-Bayes calibrated Hero & Trinket priors!
        self.sov_kb = get_architect_knowledge(catalog_name)
        self.refresh_knowledge_priors()
        # Use 1st-place-sharp BLAS predictor!
        if use_neural_value:
            self.predictor = ArchitectNumpyPredictor(
                catalog_name=catalog_name, weights_path=eff_weights
            )
        # Swap in our Comp-Aware Hybrid Positioner!
        self.positioner = ArchitectHybridPositioner(
            catalog_name=catalog_name, max_candidates=68, evaluator=self
        )
        self.reg: CompRegistry = get_comp_registry(catalog_name)
        self.planner = CompPlanner(catalog_name)
        self.comp_weight = comp_weight
        self.zero_net_weight = zero_net_weight
        self.dd_hold_weight = dd_hold_weight
        self.fast_curve_weight = fast_curve_weight
        self.zero_predictor = None
        if zero_net_weight > 0.0:
            from .zero_model import DEFAULT_ZERO_NPZ_PATH, ZeroNumpyPredictor

            zp = zero_weights_path or DEFAULT_ZERO_NPZ_PATH
            if zp and os.path.exists(zp):
                self.zero_predictor = ZeroNumpyPredictor(zp)
        self._last_plan: Optional[BuildPlan] = None
        self._plan_cache_key: Optional[Tuple] = None

        # Pre-resolve 60+ Patch 37.0.2 multi-card exponential combos
        self._pair_combos: Tuple[Tuple[int, int, float], ...] = tuple(
            (a, b, w)
            for n1, n2, w in (
                # Undead Reborn + Butcher (user's exact callout!)
                ("Snazzy Phantom", "Drustfallen Butcher", 15.0),
                ("Snazzy Phantom", "Barrier Banshee", 13.0),
                ("Snazzy Phantom", "Dead Bellringer", 12.0),
                ("Drustfallen Butcher", "Barrier Banshee", 10.0),
                ("Eternal Summoner", "Eternal Tycoon", 12.0),
                ("Eternal Summoner", "Titus Rivendare", 12.0),
                ("Plaguerunner", "Dead Bellringer", 10.0),
                # Quilboar
                ("Victorious Geomant", "Hot-Air Surveyor", 18.0),
                ("Victorious Geomant", "Geomagus Roogug", 13.0),
                ("Victorious Geomant", "Balinda Stonehearth", 15.0),
                ("Sanguine Champion", "Brann Bronzebeard", 12.0),
                ("Sanguine Champion", "Titus Rivendare", 11.0),
                ("Sanguine Champion", "Young Murk-Eye", 15.0),
                ("Turbo Hogrider", "Thorned Trailblazer", 14.0),
                ("Turbo Hogrider", "Veteran Brigand", 14.0),
                # Mechs
                ("Resourceful Robot", "Drakkari Enchanter", 18.0),
                ("Utility Drone", "Drakkari Enchanter", 16.0),
                ("Resourceful Robot", "Utility Drone", 15.0),
                ("Polarizing Beatboxer", "Resourceful Robot", 20.0),
                ("Polarizing Beatboxer", "Utility Drone", 18.0),
                ("Glambot", "Balinda Stonehearth", 13.0),
                ("Charging Czarina", "Gearfin", 11.0),
                # Beasts
                ("Goldrinn, the Great Wolf", "Titus Rivendare", 18.0),
                ("Goldrinn, the Great Wolf", "Banana Slamma", 16.0),
                ("Goldrinn, the Great Wolf", "Deathstrider", 15.0),
                ("Turquoise Skitterer", "Titus Rivendare", 15.0),
                ("Turquoise Skitterer", "Deathstrider", 15.0),
                ("Turquoise Skitterer", "Ravaging Scorpid", 14.0),
                # Dragons (Poet + scaling carries!)
                ("Persistent Poet", "Hopebringer", 16.0),
                ("Persistent Poet", "Heroic Broodmother", 18.0),
                ("Persistent Poet", "Crimson Vindicator", 17.0),
                ("Persistent Poet", "Runic Arcanist", 12.0),
                ("Hopebringer", "Heroic Broodmother", 15.0),
                ("Kalecgos, Arcane Aspect", "Brann Bronzebeard", 18.0),
                # Elementals & Demons
                ("Flaming Enforcer", "Drakkari Enchanter", 15.0),
                ("En-Djinn Blazer", "Brann Bronzebeard", 15.0),
                ("Living Azerite", "Unbound Tempest", 13.0),
                ("Soul Rewinder", "Tichondrius", 12.0),
                ("Soul Rewinder", "Eredar Escapist", 13.0),
                ("Deft Deserter", "Insatiable Ur'zul", 11.0),
                ("Deft Deserter", "Soulkeeping Jailer", 11.0),
                # Murlocs
                ("Young Murk-Eye", "Brann Bronzebeard", 20.0),
                ("Young Murk-Eye", "Drakkari Enchanter", 16.0),
                ("Young Murk-Eye", "Hackerfin", 16.0),
                ("Young Murk-Eye", "Primalfin Lookout", 13.0),
                ("Twilight Tidehunter", "Balinda Stonehearth", 13.0),
                ("Twilight Tidehunter", "Shamanic Tidecaller", 12.0),
                ("Choral Mrrrglr", "Futurefin", 18.0),
                # Pirates
                ("Sky Admiral Rogers", "Proud Privateer", 18.0),
                ("Hooktusk, Master Marauder", "Enterprising Escapee", 13.0),
                ("Ship Master Eudora", "Titus Rivendare", 12.0),
                # Aberrations
                ("Mindbender Ghur'sha", "Cutthroat K'Thir", 12.0),
                ("Mindbender Ghur'sha", "Harbinger Aph'lass", 13.0),
                ("Mindbender Ghur'sha", "Mindbending Recruiter", 11.0),
                ("Faceless Converter", "Titus Rivendare", 13.0),
                # Spells & Menagerie
                ("Felfire Conjurer", "Drakkari Enchanter", 16.0),
                ("Cataclysmic Harbinger", "Drakkari Enchanter", 16.0),
                ("Gatekeeper Amalgam", "Balinda Stonehearth", 20.0),
                ("Gatekeeper Amalgam", "Victorious Geomant", 15.0),
                ("Felfire Conjurer", "Cataclysmic Harbinger", 13.0),
            )
            if (a := self.reg.name_to_id.get(n1)) is not None
            and (b := self.reg.name_to_id.get(n2)) is not None
        )

        self._non_stack_ids: FrozenSet[int] = frozenset(
            cid
            for name in ("Brann Bronzebeard", "Drakkari Enchanter", "Soul Rewinder")
            if (cid := self.reg.name_to_id.get(name)) is not None
        )
        self._finals_dead_scaler_ids: FrozenSet[int] = frozenset(
            cid
            for name in (
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
                "Treasure Parrot",
                "Kelp Keeper",
            )
            if (cid := self.reg.name_to_id.get(name)) is not None
        )
        self._spent_bc_ids: FrozenSet[int] = frozenset(
            cid
            for name in (
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
            if (cid := self.reg.name_to_id.get(name)) is not None
        )
        self._bream_id = self.reg.name_to_id.get("Bream Counter", -1)
        self._kelp_id = self.reg.name_to_id.get("Kelp Keeper", -1)

    def get_plan(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> BuildPlan:
        key = (
            obs.turn,
            tuple(u.card_id for u in obs.board),
            tuple(c.card_id for c in obs.hand if not c.is_spell),
            tuple(obs.trinkets),
        )
        if self._plan_cache_key == key and self._last_plan is not None:
            return self._last_plan
        plan = self.planner.analyze(obs, ctx)
        self._plan_cache_key = key
        self._last_plan = plan
        return plan

    def unit_keep_priority(
        self,
        u: seaglass.Unit,
        turn: int,
        tribe_counts: Dict[str, int],
        *args,
        **kwargs,
    ) -> float:
        base = super().unit_keep_priority(u, turn, tribe_counts, *args, **kwargs)
        cid = u.card_id
        board_units = kwargs.get("board_units")
        # Drop protection on spent one-shot Battlecries unless Murk-Eye / Kelp Keeper is on board!
        if cid in self._spent_bc_ids and board_units is not None:
            has_retrig = any(bu.card_id in (self._murkeye_id, self._kelp_id) for bu in board_units)
            if not has_retrig and (u.attack + u.health) < 60:
                base *= 0.60

        # Protect active comp's lynchpins & multipliers, unlock dead Tavern scalers in Finals!
        plan = self._last_plan
        if plan is not None:
            if plan.threats.is_finals and turn >= 11 and cid in self._finals_dead_scaler_ids and (u.attack + u.health) < 160:
                return base * 0.20
            if plan.primary is not None and plan.commitment >= 0.45:
                w, role = plan.comp_card_weight(plan.primary.comp, cid, u.name)
                if role in ("lynchpin", "multiplier"):
                    base += 0.45 * w * plan.commitment
                elif role == "scaler":
                    base += 0.25 * w * plan.commitment
        return base

    def macro_and_economy_value(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = True,
        particles: Optional[SovereignOpponentParticles] = None,
        in_dist_sot: bool = False,
        sot_shop_is_real: bool = False,
    ) -> float:
        base = super().macro_and_economy_value(
            obs,
            ctx=ctx,
            end_of_turn=end_of_turn,
            particles=particles,
            in_dist_sot=in_dist_sot,
            sot_shop_is_real=sot_shop_is_real,
        )
        turn = max(1, obs.turn)
        alive_units = ctx.alive_units if ctx is not None else 8
        is_finals = alive_units <= 2 and not (ctx.next_is_ghost if ctx is not None else False)
        horizon = 0.05 if is_finals else max(0.20, min(1.0, (16.5 - turn) / 10.0))

        # Strictly lightweight, zero-leveling-distortion adjustments (`< 0.005ms`, no per-node CompPlanner call!):
        adj = 0.0
        board = obs.board
        board_ids = {u.card_id for u in board}

        # 1. Exact Patch 37.0.2 pairwise exponential engine completions (ONLY Turn 6+ so early leveling is NEVER distorted!)
        if turn >= 6 and not is_finals and self.comp_weight > 0.0:
            pair_sum = 0.0
            for c1, c2, bonus in self._pair_combos:
                if c1 in board_ids and c2 in board_ids:
                    pair_sum += bonus
            adj += self.comp_weight * min(30.0, pair_sum) * horizon

        # 1b. Clean v10 1st-Place Anti-Tier-4-Stall Discipline + Optional Grandmaster Fast Curve (`fast_curve_weight`)
        owns_candle = self._candle_id > 0 and any(tid == self._candle_id for tid in obs.trinkets)
        eff_hp = self.effective_hp(obs)
        is_ghost_now = ctx is not None and ctx.next_is_ghost
        if not owns_candle and not is_finals:
            if self.fast_curve_weight > 0.0:
                fc_safety = 1.0 if is_ghost_now else max(0.20, min(1.0, (eff_hp - 13.0) / 15.0))
                fc_tier_bonus = (0.0, 0.0, 2.0, 7.5, 16.5, 27.0, 38.0, 44.0)[min(7, max(0, int(obs.tavern_tier)))]
                adj += self.fast_curve_weight * fc_tier_bonus * horizon * fc_safety
                if len(board) >= 4:
                    if turn >= 5 and obs.tavern_tier < 3 and (eff_hp >= 22.0 or is_ghost_now):
                        adj -= self.fast_curve_weight * 8.0 * (3 - obs.tavern_tier)
                    elif turn >= 7 and obs.tavern_tier < 4 and (eff_hp >= 20.0 or is_ghost_now):
                        adj -= self.fast_curve_weight * 10.5 * (4 - obs.tavern_tier)
                    elif turn >= 9 and obs.tavern_tier < 5 and (eff_hp >= 18.0 or is_ghost_now):
                        adj -= self.fast_curve_weight * 10.0 * (5 - obs.tavern_tier)
                    elif turn >= 11 and obs.tavern_tier < 6 and (eff_hp >= 18.0 or is_ghost_now):
                        adj -= self.fast_curve_weight * 9.0
            elif len(board) >= 6:
                if turn >= 9 and obs.tavern_tier < 5 and eff_hp >= 16.0:
                    adj -= 5.5 * (5 - obs.tavern_tier)
                elif turn >= 11 and obs.tavern_tier < 6 and eff_hp >= 18.0:
                    adj -= 4.5

        # 1c. Never enter late-game combat with < 7 minions while holding a playable minion in hand (fixes g=005 P3!)
        if end_of_turn and turn >= 7 and len(board) < 7:
            if any(not c.is_spell and not c.unplayable for c in obs.hand):
                adj -= 14.0

        # 2. Non-stacking duplicate aura penalty (Golden + plain Brann/Drakkari/Soul Rewinder does nothing!)
        seen_ns: Dict[int, bool] = {}
        for u in board:
            cid = u.card_id
            if cid in self._non_stack_ids:
                if cid in seen_ns:
                    if seen_ns[cid] or u.is_golden:
                        adj -= 9.5
                else:
                    seen_ns[cid] = bool(u.is_golden)

        # 3. Protect Bream Counter in hand when handbuff Murloc synergy is active!
        if self._bream_id in board_ids and self.has_hand_buff_synergy(obs) and turn <= 12:
            adj -= 7.5

        # 4. Top-2 Finals: unlock dead Tavern-only scalers so the bot swaps them for Scam/Tech!
        if is_finals and turn >= 11:
            for u in board:
                if u.card_id in self._finals_dead_scaler_ids and (u.attack + u.health) < 150:
                    adj -= 11.0

        # 5. Pure Patch 37.0.2 `DarkDiscovery` Turn 9-10+ Option Value (`FirstTwo` tribe-matched Tier 5/6!)
        if self.dd_hold_weight > 0.0 and not is_finals:
            dd_left = int(obs.dark_discoveries_left)
            if dd_left > 0:
                hp_gate = 1.0 if (eff_hp >= 22.0 or (ctx is not None and ctx.next_is_ghost)) else max(0.15, (eff_hp - 10.0) / 12.0)
                if turn <= 6:
                    adj += self.dd_hold_weight * 28.0 * dd_left * hp_gate
                elif turn == 7:
                    adj += self.dd_hold_weight * 24.0 * dd_left * hp_gate
                elif turn == 8:
                    adj += self.dd_hold_weight * 18.0 * dd_left * hp_gate
                elif turn == 9:
                    adj += self.dd_hold_weight * 8.0 * dd_left * hp_gate

        return base + adj

    def evaluate_turn_end(
        self,
        tavern: seaglass.TavernGame,
        particles: SovereignOpponentParticles,
        ctx: Optional[seaglass.LobbyContext] = None,
        combats_per_particle: int = 14,
        board_override: Optional[List[seaglass.Unit]] = None,
        pre_positioned: bool = False,
    ) -> float:
        base = super().evaluate_turn_end(
            tavern,
            particles,
            ctx=ctx,
            combats_per_particle=combats_per_particle,
            board_override=board_override,
            pre_positioned=pre_positioned,
        )
        obs_pre = tavern.observe()
        alive_units = ctx.alive_units if ctx is not None else 8
        is_ghost = particles.is_ghost or (ctx.next_is_ghost if ctx is not None else False)
        hp_now = self.effective_hp(obs_pre)

        # Extra 1st-Place Finals Override:
        #   In Top-2 Finals (`alive_units <= 2` and not ghost), winning THIS combat IS 1st place!
        #   Add a direct combat win-rate amplifier against the fresh observed opponent board.
        extra = 0.0
        if not is_ghost and board_override is None and alive_units <= 2 and particles.boards:
            cg = tavern.copy()
            if not pre_positioned:
                self.positioner.fast_reposition_game(cg, obs_pre, particles)
            cg.reseed((obs_pre.turn * 65537 + 17) | 1)
            if cg.is_legal(seaglass.TavernAction.end_turn()):
                cg.end_turn()
            dist = cg.equity_against(
                particles.boards[0],
                opponent_tier=particles.tier,
                n=max(16, combats_per_particle),
                base_seed=obs_pre.turn * 2027 + 7,
            )
            extra += 48.0 * dist.a_win_rate + 16.0 * dist.draw_rate + 0.55 * dist.damage_mean

        if self.zero_predictor is not None and self.zero_predictor.is_ready and self.zero_net_weight > 0.0:
            zu, _, zfp = self.zero_predictor.predict_batch_full([obs_pre], ctx=ctx, physics_list=None, end_of_turn=True)
            extra += self.zero_net_weight * (float(zu[0]) + (1.5 * float(zfp[0]) if obs_pre.turn <= 12 else 0.0))

        return base + extra


class ArchitectBot(SovereignBot):
    """
    Composition-Driven Superhuman Bot (`ArchitectBot v7+`):
      - Inherits SovereignBot's full tactical completeness (zero regressions!)
      - Layers our 26-Comp Planner, Pairwise Exponential Engine Evaluator,
        Comp-Specific Tavern Sequencing Macros, Comp-Aware Positioning Invariants,
        1st-Place Lobby-Boss Objective, Finals Counter-Tech Solver, and deeper search!
    """

    def __init__(
        self,
        catalog_name: str = "full",
        beam_width: int = 34,
        max_epoch_depth: int = 12,
        top_k_combat_evals: int = 12,
        refresh_samples: int = 8,
        num_particles: int = 4,
        seed: int = 2029,
        use_neural_value: bool = True,
        neural_weight: float = 0.24,
        comp_weight: float = 0.12,
        weights_path: Optional[os.PathLike | str] = None,
        zero_net_weight: float = 0.0,
        zero_weights_path: Optional[os.PathLike | str] = None,
        dd_hold_weight: float = 0.0,
        fast_curve_weight: float = 0.0,
    ) -> None:
        w_str = str(weights_path) if weights_path is not None else None
        zw_str = str(zero_weights_path) if zero_weights_path is not None else None
        arch_eval = ArchitectHybridEvaluator(
            catalog_name=catalog_name,
            use_physics_rollout=True,
            use_neural_value=use_neural_value,
            neural_weight=neural_weight,
            weights_path=w_str,
            comp_weight=comp_weight,
            zero_net_weight=zero_net_weight,
            zero_weights_path=zw_str,
            dd_hold_weight=dd_hold_weight,
            fast_curve_weight=fast_curve_weight,
        )
        super().__init__(
            catalog_name=catalog_name,
            beam_width=beam_width,
            max_epoch_depth=max_epoch_depth,
            top_k_combat_evals=top_k_combat_evals,
            refresh_samples=refresh_samples,
            num_particles=num_particles,
            seed=seed,
            evaluator=arch_eval,
            weights_path=w_str,
        )
        self.arch_eval: ArchitectHybridEvaluator = arch_eval
        self.sov_kb = arch_eval.sov_kb
        self.reg: CompRegistry = arch_eval.reg

        def _id(name: str) -> int:
            return self.reg.name_to_id.get(name, -1)

        self._snazzy_id = _id("Snazzy Phantom")
        self._banshee_id = _id("Barrier Banshee")
        self._bellringer_id = _id("Dead Bellringer")
        self._beatboxer_id = _id("Polarizing Beatboxer")
        self._duplicator_id = _id("Drone Duplicator")
        self._geomant_id = _id("Victorious Geomant")
        self._surveyor_id = _id("Hot-Air Surveyor")
        self._roogug_id = _id("Geomagus Roogug")
        self._amalgam_id = _id("Gatekeeper Amalgam")
        self._harbinger_id = _id("Cataclysmic Harbinger")
        self._lionfish_id = _id("Lurking Lionfish")
        self._shark_id = _id("Snarky Shark")
        self._living_prison_id = _id("Living Prison")
        self._conniver_id = _id("Greedy Conniver")
        self._elite_nav_id = _id("Elite Navigator")
        self._behemoth_id = _id("Arcane Behemoth")

        tag_names, tag_table = seaglass.card_tags()
        t_idx = {n: i for i, n in enumerate(tag_names)}
        rally_col = t_idx.get("rally")
        self._rally_ids: FrozenSet[int] = (
            frozenset(cid for cid, row in tag_table.items() if row[rally_col] > 0.5)
            if rally_col is not None
            else frozenset()
        )
        self._last_plan: Optional[BuildPlan] = None
        self._enable_two_turn_lookahead: bool = True

    def act_in_lobby(
        self,
        lobby: seaglass.LobbyGame,
        player: int,
    ) -> seaglass.TavernAction:
        obs = lobby.observe(player)
        ctx = lobby.context(player)
        if lobby.phase == "turn":
            self._last_plan = self.arch_eval.get_plan(obs, ctx)
        return super().act_in_lobby(lobby, player)

    def choose_action_for_tavern(
        self,
        belief_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        player: int = 0,
        ctx: Optional[seaglass.LobbyContext] = None,
        lobby_id: int = 0,
    ) -> seaglass.TavernAction:
        # Boost search budget dynamically in Top-2 Finals where 1 combat decides 1st place!
        alive_units = ctx.alive_units if ctx is not None else 8
        orig_ref = self.refresh_samples
        if alive_units <= 2 and belief_game.observe().turn >= 9:
            self.refresh_samples = max(orig_ref, 10)
        try:
            return super().choose_action_for_tavern(
                belief_game, legal, player=player, ctx=ctx, lobby_id=lobby_id
            )
        finally:
            self.refresh_samples = orig_ref

    def _comp_trinket_synergy(
        self, tname: str, obs: seaglass.TavernObservation, plan: Optional[BuildPlan]
    ) -> float:
        """Small comp-specific synergy nudge for Turn 6/9 Trinket selection."""
        if plan is None or plan.primary is None or plan.commitment < 0.35:
            return 0.0
        cname = plan.primary.comp.name
        c_tribes = plan.primary.comp.required_tribes
        c_mult = plan.commitment
        if tname == "Drakkari Portrait" and any(
            k in cname for k in ("Mech", "Spell", "Harbinger", "Quilboar", "Murk-Eye", "Elemental")
        ):
            return 7.0 * c_mult
        if tname == "Titus Portrait" and any(
            k in cname for k in ("Beast", "Goldrinn", "Skitterer", "Undead", "Eternal", "Deathrattle", "Aberration")
        ):
            return 7.0 * c_mult
        if tname == "Bronzebeard Portrait" and any(
            k in cname for k in ("Murloc", "Kalecgos", "Battlecry", "Demon", "Elemental", "Pirate")
        ):
            return 7.0 * c_mult
        if tname in ("Nomi Sticker", "Azerite Portrait") and "elemental" in c_tribes:
            return 6.5 * c_mult
        if tname == "Great Boar Sticker" and "quilboar" in c_tribes:
            return 5.5 * c_mult
        if tname == "Curator Sticker" and ("dragon" in c_tribes or "Amalgam" in cname or "Menagerie" in cname):
            return 5.0 * c_mult
        return 0.0

    def _choose_best_trinket(
        self,
        base_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        particles: SovereignOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
    ) -> seaglass.TavernAction:
        """
        CRN-Paired (`M = 3` seeds!) 2-Turn Exact Rust Physics Rollout for Turn 6 & Turn 9 Trinkets
        with Patch 37.0.2 Empirical-Bayes priors and Comp-Synergy conditioning!
        """
        obs0 = base_game.observe()
        plan = self.arch_eval.get_plan(obs0, ctx)
        ehp0 = self.evaluator.effective_hp(obs0)
        alive_units = ctx.alive_units if ctx is not None else 8
        owns_souvenir = self.kb.souvenir_stand_id > 0 and any(
            t == self.kb.souvenir_stand_id for t in obs0.trinkets
        )
        avg_board_stats = (
            sum(u.attack + u.health for u in obs0.board) / len(obs0.board)
            if obs0.board
            else 0.0
        )
        no_type_cnt = sum(1 for u in obs0.board if len(u.tribes) == 0)
        no_type_stats = sum(u.attack + u.health for u in obs0.board if len(u.tribes) == 0)
        has_high_tier_carry = any(
            self.kb.engine_bonus_by_id.get(u.card_id, 0.0) >= 12.0 and u.tavern_tier >= 5
            for u in obs0.board
        )
        aberration_cnt = sum(1 for u in obs0.board if "aberration" in u.tribes or "all" in u.tribes)
        dr_cnt = sum(
            1
            for u in obs0.board
            if (self.meta.profile(u.card_id) is not None and self.meta.profile(u.card_id).has_deathrattle)
            or u.extra_deathrattles
        )
        stackable_greater_names = {
            "Yogg-Tastic Pastry",
            "Inductive Gyroblade",
            "Tableware Portrait",
            "Wax Lance",
            "Sinister Invitation",
            "Azeroth Model Globe",
            "Sky Golem Portrait",
            "Morgl Portrait",
            "Pagle's Fishing Rod",
            "Bronzebeard Portrait",
            "Drakkari Portrait",
            "Titus Portrait",
            "Nomi Sticker",
            "Curator Sticker",
        }

        # M = 3 CRN seeds shared identically across all offered Trinkets (lower variance than Sovereign's M = 2!)
        crn_seeds = [
            self.rng.getrandbits(63) | 1,
            self.rng.getrandbits(63) | 1,
            self.rng.getrandbits(63) | 1,
        ]

        best_act = legal[0]
        best_val = -1e18

        for act in legal:
            if act.kind != "choose_trinket":
                continue
            opt_idx, _ = act.args
            t_prior = 0.0
            tid = -1
            if opt_idx is not None and opt_idx < len(obs0.trinket_options):
                tid = obs0.trinket_options[opt_idx]
                tinfo = self.kb.trinket_info.get(tid)
                if tinfo is not None:
                    tname, _is_g, _tcost, impl = tinfo
                    if not impl:
                        t_prior -= 14.0
                    t_prior += 0.80 * self.sov_kb.trinket_prior_score(tname)
                    t_prior += self._comp_trinket_synergy(tname, obs0, plan)
                    if tname in (
                        "Souvenir Stand",
                        "Worn Treasure Map",
                        "Stuffed Coin Purse",
                        "Trip Vouchers",
                        "Goblin Wallet",
                        "Transcribing Typewriter",
                        "Bartend-o-Tron's Oilcan",
                        "Book of Medivh",
                    ):
                        if ehp0 <= 18.0 or (alive_units <= 2 and obs0.turn <= 6):
                            t_prior -= 18.0
                    elif tname == "Mysterious Orb" and alive_units <= 2 and obs0.turn <= 6:
                        t_prior += 18.0
                    elif tname == "Shrine of Evolution" and avg_board_stats >= 10.0:
                        t_prior -= 24.0
                    elif tname == "Safety Patch" and ehp0 <= 14.0:
                        t_prior += 8.0
                    elif tname == "Ironforge Anvil":
                        if no_type_stats < 28:
                            t_prior -= 24.0
                        elif no_type_cnt >= 2:
                            t_prior += 5.5 * no_type_cnt
                    elif tname == "Maldraxxus Dagger":
                        if has_high_tier_carry and ehp0 >= 26.0:
                            t_prior += 6.0
                        else:
                            t_prior -= 22.0
                    elif tname in ("Mask of Ancient Ones", "Evil Experiment"):
                        if obs0.deity == "none" and aberration_cnt < 2:
                            t_prior -= 25.0
                    elif tname == "Soul Fermenter" and dr_cnt < 3:
                        t_prior -= 18.0
                    if owns_souvenir and _is_g:
                        if tname in stackable_greater_names:
                            t_prior += 12.0
                        elif tname in ("Manipulator Portrait", "Safety Patch", "Ghastly Sticker"):
                            t_prior -= 12.0

            rollout_sum = 0.0
            for m_idx, r_seed in enumerate(crn_seeds):
                sim = base_game.copy()
                sim.reseed(r_seed)
                sim.step(act)

                for _ in range(3):
                    s_obs = sim.observe()
                    if s_obs.discover_pending is not None:
                        d_legal = sim.legal_actions()
                        if d_legal:
                            pick = self._evaluate_discover_options(
                                sim, d_legal, particles, ctx, allow_shop_continuation=False
                            )
                            sim.step(pick)
                            self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
                    elif s_obs.trinket_options:
                        t_legal = sim.legal_actions()
                        if t_legal:
                            sim.step(t_legal[0])
                    else:
                        break

                self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
                if tid == self.kb.guiding_candle_id and sim.observe().gold >= 4:
                    ref0 = seaglass.TavernAction.refresh()
                    if sim.is_legal(ref0):
                        sim.step(ref0)

                sim_t0 = self._rollout_shop_consumer(
                    sim, ctx, particles=particles, max_steps=4, crn_seed=(r_seed ^ 0x13579BDF) | 1
                )
                v_turn0 = self.evaluator.evaluate_turn_end(
                    sim_t0, particles, ctx=ctx, combats_per_particle=8
                )

                v_turn1_macro = 0.0
                if particles.boards:
                    sim_t1 = sim_t0.copy()
                    self.positioner.fast_reposition_game(sim_t1, sim_t1.observe(), particles)
                    c_seed = (obs0.turn * 262147 + m_idx * 4099 + 5) | 1
                    sim_t1.reseed(c_seed)
                    if sim_t1.is_legal(seaglass.TavernAction.end_turn()):
                        sim_t1.end_turn()
                    opp_b = particles.boards[m_idx % len(particles.boards)]
                    sim_t1.resolve_combat_against(
                        opp_b, opponent_tier=particles.tier, seed=c_seed
                    )
                    sim_t1.start_turn()
                    for _ in range(2):
                        if sim_t1.observe().discover_pending is not None:
                            self._fast_resolve_chained_discover(sim_t1, particles=particles, ctx=ctx)
                            self._play_newly_gained_card_in_sim(sim_t1, particles=particles, ctx=ctx)
                        else:
                            break
                    if tid == self.kb.guiding_candle_id:
                        ref1 = seaglass.TavernAction.refresh()
                        if sim_t1.is_legal(ref1):
                            sim_t1.step(ref1)
                    sim_t1 = self._rollout_shop_consumer(
                        sim_t1, ctx, particles=particles, max_steps=4, crn_seed=(c_seed ^ 0x2468ACE1) | 1
                    )
                    self.positioner.fast_reposition_game(sim_t1, sim_t1.observe(), particles)
                    if sim_t1.is_legal(seaglass.TavernAction.end_turn()):
                        sim_t1.end_turn()
                    sim_t1.resolve_combat_against(
                        opp_b, opponent_tier=particles.tier, seed=c_seed + 2
                    )
                    sim_t1.start_turn()
                    v_turn1_macro = self.evaluator.evaluate_fast(
                        sim_t1.observe(),
                        ctx=ctx,
                        end_of_turn=False,
                        particles=particles,
                        in_dist_sot=True,
                    )

                rollout_sum += 0.65 * v_turn0 + 0.35 * v_turn1_macro

            total_val = (rollout_sum / len(crn_seeds)) + t_prior
            if total_val > best_val:
                best_val = total_val
                best_act = act

        return best_act

    def _play_newly_gained_card_in_sim(
        self,
        sim: seaglass.TavernGame,
        particles: Optional[SovereignOpponentParticles] = None,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> None:
        """Unlock Finals scam/tech swaps over dead Tavern scalers on a full 7-board before falling back to parent."""
        obs_pre = sim.observe()
        alive_units = ctx.alive_units if ctx is not None else 8
        if (
            alive_units <= 2
            and obs_pre.turn >= 10
            and obs_pre.discover_pending is None
            and not obs_pre.trinket_options
            and len(obs_pre.board) == 7
            and obs_pre.hand
        ):
            new_idx = len(obs_pre.hand) - 1
            new_card = obs_pre.hand[new_idx]
            if (
                not new_card.is_spell
                and not new_card.unplayable
                and (new_card.venomous or new_card.card_id in self.reg.scam_ids)
            ):
                dead_slot = None
                worst_st = 1e18
                enemy_huge = particles is not None and particles.max_enemy_hp >= 350
                for bi, bu in enumerate(obs_pre.board):
                    if bu.venomous or bu.card_id in self.reg.scam_ids:
                        continue
                    bu_st = bu.attack + bu.health
                    is_dead_sc = bu.card_id in self.arch_eval._finals_dead_scaler_ids and bu_st < 180
                    is_weak_vs_huge = enemy_huge and not bu.divine_shield and bu_st < 280
                    if is_dead_sc or is_weak_vs_huge:
                        st = bu_st * (0.5 if is_dead_sc else 1.0) + (40.0 if bu.divine_shield else 0.0)
                        if st < worst_st:
                            worst_st = st
                            dead_slot = bi
                if dead_slot is not None:
                    sell_act = seaglass.TavernAction.sell(dead_slot)
                    if sim.is_legal(sell_act):
                        sim.step(sell_act)
                        obs_mid = sim.observe()
                        if obs_mid.hand:
                            p_act = seaglass.TavernAction.play(len(obs_mid.hand) - 1, min(dead_slot, len(obs_mid.board)))
                            if sim.is_legal(p_act):
                                n_hand_mid = len(obs_mid.hand)
                                sim.step(p_act)
                                self._drain_post_play_rewards(sim, n_hand_mid - 1, particles=particles, ctx=ctx)
                                return
        super()._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)

    def _two_turn_lookahead_value(
        self,
        eval_game: seaglass.TavernGame,
        particles: SovereignOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
        crn_seeds: Tuple[int, ...],
    ) -> float:
        """Paired-seed 2-turn physics lookahead: simulate Turn t combat -> Turn t+1 shop -> Turn t+1 EOT!"""
        total = 0.0
        for m_idx, r_seed in enumerate(crn_seeds):
            sim = eval_game.copy()
            sim.reseed(r_seed)
            self.positioner.fast_reposition_game(sim, sim.observe(), particles)
            if sim.is_legal(seaglass.TavernAction.end_turn()):
                sim.end_turn()
            opp_b = particles.boards[m_idx % len(particles.boards)]
            sim.resolve_combat_against(opp_b, opponent_tier=particles.tier, seed=r_seed)
            obs_post = sim.observe()
            if obs_post.health <= 0:
                total += -260.0
                continue
            sim.start_turn()
            # Drain any pending Turn t+1 Start-of-Turn discover/trinket cleanly
            for _ in range(2):
                s_obs = sim.observe()
                if s_obs.discover_pending is not None:
                    self._fast_resolve_chained_discover(sim, particles=particles, ctx=ctx)
                    self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
                elif s_obs.trinket_options:
                    t_leg = sim.legal_actions()
                    if t_leg:
                        sim.step(t_leg[0])
                else:
                    break
            sim_t1 = self._rollout_shop_consumer(
                sim, ctx, particles=particles, max_steps=4, crn_seed=(r_seed ^ 0x51A7E3B9) | 1
            )
            v_t1 = self.evaluator.evaluate_turn_end(
                sim_t1, particles, ctx=ctx, combats_per_particle=6
            )
            total += v_t1
        return total / max(1, len(crn_seeds))

    def _search_shop_epoch(
        self,
        root_game: seaglass.TavernGame,
        particles: SovereignOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
    ) -> Tuple[float, List[QueuedStep], List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]]]:
        root_obs = root_game.observe()
        alive_units = ctx.alive_units if ctx is not None else 8
        is_finals = alive_units <= 2 and not (ctx.next_is_ghost if ctx is not None else False)
        root_fast = self.evaluator.evaluate_fast(
            root_obs, ctx=ctx, end_of_turn=False, particles=particles
        )
        transposition_best: Dict[Tuple, float] = {_canonical_state_key(root_obs): root_fast}

        beam: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = [
            (root_fast, root_game, root_obs, [])
        ]

        raw_terminals: List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, bool, List[QueuedStep]]] = []
        roll_prefixes: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []

        # Finals Dead-Scaler -> Refresh Roll Prefix:
        #   In Top-2 Finals on a full 7-board, explicitly allow selling a dead Tavern scaler (`Brann`, `Drakkari`,
        #   `Rewinder`, `Kalecgos`, `Rogers`, etc.) to fund `Refresh -> Buy(Leeroy/Bile Spitter/Spore/Sin'dorei)`!
        if (
            is_finals
            and root_obs.turn >= 10
            and len(root_obs.board) >= 7
            and root_obs.gold >= max(1, root_obs.refresh_cost + root_obs.minion_cost - 1)
            and root_obs.discover_pending is None
            and not root_obs.trinket_options
        ):
            dead_slot = None
            worst_st = 1e18
            for bi, bu in enumerate(root_obs.board):
                if bu.card_id in self.arch_eval._finals_dead_scaler_ids and (bu.attack + bu.health) < 150:
                    st = bu.attack + bu.health + (40.0 if bu.divine_shield else 0.0)
                    if st < worst_st:
                        worst_st = st
                        dead_slot = bi
            if dead_slot is not None:
                sell_act = seaglass.TavernAction.sell(dead_slot)
                if root_game.is_legal(sell_act):
                    sg = root_game.copy()
                    sg.reseed(self.rng.getrandbits(63) | 1)
                    sig0 = _plan_validation_key(root_obs)
                    sg.step(sell_act)
                    s_obs = sg.observe()
                    if self._can_productive_refresh(s_obs):
                        s_fast = self.evaluator.evaluate_fast(
                            s_obs, ctx=ctx, end_of_turn=False, particles=particles
                        ) + 12.0  # Ensure Finals dead-scaler roll prefix makes top_prefixes!
                        roll_prefixes.append((s_fast, sg, s_obs, [(_action_key(sell_act), sig0, False)]))

        zero_cost_spells = sum(
            1 for c in root_obs.hand if c.is_spell and c.spell_cost == 0 and not c.unplayable
        )
        eff_depth = min(16, self.max_epoch_depth + zero_cost_spells)
        for _depth in range(eff_depth):
            raw_children: List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []

            for b_fast, g, obs, path in beam:
                obs_sig = _plan_validation_key(obs)
                end_step: QueuedStep = (("end_turn", None, None), obs_sig, False)
                raw_terminals.append((g, obs, True, path + [end_step]))

                if path and obs.discover_pending is None and not obs.trinket_options and self._can_productive_refresh(obs):
                    roll_prefixes.append((b_fast, g, obs, path))

                if (
                    not obs.is_frozen
                    and obs.shop
                    and (obs.gold < obs.minion_cost or (obs.gold <= 3 and len(obs.board) >= 7))
                ):
                    fz_act = seaglass.TavernAction.toggle_freeze()
                    if g.is_legal(fz_act):
                        g_fz = g.copy()
                        g_fz.step(fz_act)
                        fz_obs = g_fz.observe()
                        fz_step: QueuedStep = (("toggle_freeze", None, None), obs_sig, False)
                        fz_end: QueuedStep = (
                            ("end_turn", None, None),
                            _plan_validation_key(fz_obs),
                            False,
                        )
                        raw_terminals.append((g_fz, fz_obs, True, path + [fz_step, fz_end]))

                if obs.discover_pending is not None or obs.trinket_options:
                    raw_terminals.append((g, obs, False, path))
                    continue

                legal = prune_architect_actions(
                    obs, g.legal_actions(), self.arch_eval, self.kb, particles=particles, ctx=ctx
                )
                n_hand_pre = len(obs.hand)
                n_shop_pre = len(obs.shop)
                for act in legal:
                    k = act.kind
                    if k in ("end_turn", "toggle_freeze", "refresh", "dark_discovery", "pass"):
                        continue
                    if k in ("hero_power", "hero_power_2") and self._is_stochastic_hero_power(g, obs, act):
                        continue

                    child = g.copy()
                    child.reseed(self.rng.getrandbits(63) | 1)
                    ended = child.step(act)
                    c_obs = child.observe()

                    is_stoch_step = False
                    if (
                        k in ("play", "activate", "sell", "play_on_shop")
                        and c_obs.discover_pending is None
                    ):
                        gained_hand_card = (
                            len(c_obs.hand) >= n_hand_pre
                            if k in ("play", "play_on_shop")
                            else len(c_obs.hand) > n_hand_pre
                        )
                        shop_mutated = (
                            len(c_obs.shop) != n_shop_pre and k != "play_on_shop"
                        )
                        board_transformed = (
                            k != "sell"
                            and len(c_obs.board) == len(obs.board)
                            and [u.card_id for u in c_obs.board] != [u.card_id for u in obs.board]
                        )
                        if gained_hand_card or shop_mutated or board_transformed:
                            probe = g.copy()
                            probe.reseed(999983)
                            probe.step(act)
                            if _decision_skeleton(probe.observe()) != _decision_skeleton(c_obs):
                                is_stoch_step = True

                    q_step: QueuedStep = (_action_key(act), obs_sig, is_stoch_step)
                    if ended:
                        raw_terminals.append((child, c_obs, True, path + [q_step]))
                    elif is_stoch_step:
                        raw_terminals.append((child, c_obs, False, path + [q_step]))
                    else:
                        raw_children.append((child, c_obs, path + [q_step]))

                raw_children.extend(
                    self._expand_cycle_macros(g, obs, path, particles=particles, ctx=ctx)
                )

            if not raw_children:
                break

            dedup_children: List[Tuple[Tuple, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []
            seen_layer: set[Tuple] = set()
            for child, c_obs, c_path in raw_children:
                ckey = _canonical_state_key(c_obs)
                if ckey in seen_layer:
                    continue
                seen_layer.add(ckey)
                dedup_children.append((ckey, child, c_obs, c_path))

            child_scores = self.evaluator.evaluate_fast_batch(
                [item[2] for item in dedup_children],
                ctx=ctx,
                end_of_turn=False,
                particles=particles,
            )

            next_beam: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []
            for (ckey, child, c_obs, c_path), c_fast in zip(dedup_children, child_scores):
                prev = transposition_best.get(ckey)
                if prev is not None and c_fast <= prev + 1e-6:
                    continue
                transposition_best[ckey] = c_fast
                next_beam.append((c_fast, child, c_obs, c_path))

            if not next_beam:
                break
            next_beam.sort(key=lambda x: x[0], reverse=True)
            beam = next_beam[: self.beam_width]

        eot_indices = [i for i, item in enumerate(raw_terminals) if item[2]]
        open_indices = [i for i, item in enumerate(raw_terminals) if not item[2]]
        term_scores = [0.0] * len(raw_terminals)

        if eot_indices:
            vals = self.evaluator.evaluate_fast_batch(
                [raw_terminals[i][1] for i in eot_indices],
                ctx=ctx,
                end_of_turn=True,
                particles=particles,
            )
            for idx, v in zip(eot_indices, vals):
                term_scores[idx] = v
        if open_indices:
            vals = self.evaluator.evaluate_fast_batch(
                [raw_terminals[i][1] for i in open_indices],
                ctx=ctx,
                end_of_turn=False,
                particles=particles,
            )
            for idx, v in zip(open_indices, vals):
                term_scores[idx] = v

        ranked_terms: List[Tuple[float, seaglass.TavernGame, List[QueuedStep]]] = []
        seen_term_keys: set[Tuple] = set()
        for t_score, (t_game, t_obs, t_eot, t_path) in sorted(
            zip(term_scores, raw_terminals), key=lambda x: x[0], reverse=True
        ):
            t_sig = (_canonical_state_key(t_obs), t_eot)
            if t_sig in seen_term_keys:
                continue
            seen_term_keys.add(t_sig)
            ranked_terms.append((t_score, t_game, t_path))
            if len(ranked_terms) >= self.top_k_combat_evals:
                break

        best_score = -1e18
        default_end: QueuedStep = (
            ("end_turn", None, None),
            _plan_validation_key(root_obs),
            False,
        )
        best_path: List[QueuedStep] = [default_end]
        cpp = 22 if is_finals else 14

        evaluated_cands: List[Tuple[float, seaglass.TavernGame, List[QueuedStep]]] = []
        for _, cand_game, cand_path in ranked_terms:
            eval_game = cand_game
            cand_obs = cand_game.observe()
            if cand_obs.discover_pending is not None:
                eval_game = cand_game.copy()
                eval_game.reseed(self.rng.getrandbits(63) | 1)
                d_legal = eval_game.legal_actions()
                if d_legal:
                    d_pick = self._evaluate_discover_options(
                        eval_game, d_legal, particles, ctx, allow_shop_continuation=False
                    )
                    eval_game.step(d_pick)
                    self._play_newly_gained_card_in_sim(eval_game, particles=particles, ctx=ctx)
                eval_game = self._rollout_shop_consumer(eval_game, ctx, particles=particles, max_steps=3)
            elif cand_path and cand_path[-1][2]:
                eval_game = self._rollout_shop_consumer(cand_game, ctx, particles=particles, max_steps=3)

            full_score = self.evaluator.evaluate_turn_end(
                eval_game,
                particles,
                ctx=ctx,
                combats_per_particle=cpp,
            )
            evaluated_cands.append((full_score, eval_game, cand_path))
            if full_score > best_score:
                best_score = full_score
                best_path = cand_path

        # True 2-Turn CRN Physics Lookahead Reranking on critical midgame macro forks (`4 <= turn <= 10`)!
        # Only triggers when >= 2 genuinely distinct competing plans (different tavern_tier or board) are within 14 pts.
        if (
            self._enable_two_turn_lookahead
            and 4 <= root_obs.turn <= 10
            and not is_finals
            and particles.boards
            and len(evaluated_cands) >= 2
        ):
            evaluated_cands.sort(key=lambda x: x[0], reverse=True)
            top0_score = evaluated_cands[0][0]
            finalists: List[Tuple[float, seaglass.TavernGame, List[QueuedStep]]] = [evaluated_cands[0]]
            seen_macro = {
                (
                    evaluated_cands[0][1].observe().tavern_tier,
                    tuple(sorted(u.card_id for u in evaluated_cands[0][1].observe().board)),
                )
            }
            for sc, eg, ep in evaluated_cands[1:]:
                if sc < top0_score - 14.0:
                    break
                eo = eg.observe()
                mkey = (eo.tavern_tier, tuple(sorted(u.card_id for u in eo.board)))
                if mkey not in seen_macro:
                    seen_macro.add(mkey)
                    finalists.append((sc, eg, ep))
                    if len(finalists) >= 3:
                        break
            if len(finalists) >= 2:
                # Run 2-turn lookahead if at least two finalists differ in tier OR are close (<= 9.0 pts)!
                tiers = {f[1].observe().tavern_tier for f in finalists}
                if len(tiers) >= 2 or (top0_score - finalists[1][0]) <= 9.0:
                    crn_2t = (self.rng.getrandbits(63) | 1, self.rng.getrandbits(63) | 1)
                    best_blend = -1e18
                    best_t0_for_winner = best_score
                    for sc_t0, eg, ep in finalists:
                        v_t1 = self._two_turn_lookahead_value(eg, particles, ctx, crn_2t)
                        blend = 0.68 * sc_t0 + 0.32 * v_t1
                        if blend > best_blend:
                            best_blend = blend
                            best_t0_for_winner = sc_t0
                            best_path = ep
                    # Keep score at least `top0_score` so a 2-turn-winning Upgrade/Pivot isn't undercut by 1-turn Refresh!
                    best_score = max(top0_score, best_t0_for_winner)

        top_prefixes = self._select_stratified_roll_prefixes(
            roll_prefixes, root_fast=root_fast, root_obs=root_obs, max_prefixes=4
        )
        return best_score, best_path, top_prefixes

    def _try_macro_chain(
        self,
        g: seaglass.TavernGame,
        path: List[QueuedStep],
        acts: Sequence[seaglass.TavernAction],
    ) -> Optional[Tuple[seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]]:
        """Safely execute a multi-step comp macro onto a cloned game, returning `(child, obs, new_path)`."""
        cur = g.copy()
        cur.reseed(self.rng.getrandbits(63) | 1)
        new_path = list(path)
        for act in acts:
            if not cur.is_legal(act):
                return None
            sig = _plan_validation_key(cur.observe())
            cur.step(act)
            new_path.append((_action_key(act), sig, False))
            if cur.observe().discover_pending is not None:
                break
        return (cur, cur.observe(), new_path)

    def _expand_cycle_macros(
        self,
        g: seaglass.TavernGame,
        obs: seaglass.TavernObservation,
        path: List[QueuedStep],
        particles: Optional[SovereignOpponentParticles] = None,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]]:
        # Start with SovereignBot's full-board Sell+Play cycle macros
        results = super()._expand_cycle_macros(g, obs, path, particles=particles, ctx=ctx)

        board = obs.board
        hand = obs.hand
        shop = obs.shop
        n_board = len(board)
        if n_board == 0:
            return results

        # =====================================================================
        # Comp-Specific Tavern Sequencing Macros (unique to ArchitectBot!)
        # =====================================================================

        # Macro 1: Undead Tavern Reborn (`Snazzy Phantom` right-most carry prep BEFORE `Dead Bellringer` / `Butchering`!)
        has_snazzy = any(u.card_id == self._snazzy_id for u in board)
        if has_snazzy and n_board >= 2:
            # Find best Undead carry to sit right-most
            best_carry_idx = None
            best_c_sc = -1e18
            for i, u in enumerate(board):
                if u.card_id == self._bellringer_id:
                    continue
                if "undead" in u.tribes or "all" in u.tribes:
                    csc = (60.0 if u.card_id == self._banshee_id else 0.0) + (
                        30.0 if u.divine_shield else 0.0
                    ) + 0.3 * (u.attack + u.health)
                    if csc > best_c_sc:
                        best_c_sc = csc
                        best_carry_idx = i
            if best_carry_idx is not None and best_carry_idx != n_board - 1:
                prep = seaglass.TavernAction.reposition(best_carry_idx, n_board - 1)
                # (1a) Prep + Dead Bellringer on a Reborn/Plaguerunner Undead
                for bi, bu in enumerate(board):
                    if bu.card_id == self._bellringer_id and not bu.activated_this_turn and obs.gold >= 1:
                        # After moving best_carry_idx -> n_board-1, find target index
                        for ti, tu in enumerate(board):
                            if ti in (bi, best_carry_idx):
                                continue
                            if ("undead" in tu.tribes or "all" in tu.tribes) and (tu.reborn or tu.name == "Plaguerunner"):
                                new_bi = bi - 1 if bi > best_carry_idx else bi
                                new_ti = ti - 1 if ti > best_carry_idx else ti
                                res = self._try_macro_chain(
                                    g, path, (prep, seaglass.TavernAction.activate(new_bi, new_ti))
                                )
                                if res is not None:
                                    results.append(res)
                                break
                # (1b) Prep + Butchering from hand on a Reborn/Plaguerunner Undead
                for hi, hc in enumerate(hand):
                    if hc.is_spell and hc.name == "Butchering":
                        for ti, tu in enumerate(board):
                            if ti == best_carry_idx:
                                continue
                            if ("undead" in tu.tribes or "all" in tu.tribes) and (tu.reborn or tu.name == "Plaguerunner"):
                                new_ti = ti - 1 if ti > best_carry_idx else ti
                                res = self._try_macro_chain(
                                    g, path, (prep, seaglass.TavernAction.play(hi, new_ti))
                                )
                                if res is not None:
                                    results.append(res)
                                break

        # Macro 2: `Drone Duplicator` Activate-before-Magnetize!
        for bi, bu in enumerate(board):
            if bu.card_id == self._duplicator_id and not bu.activated_this_turn and obs.gold >= 1:
                for hi, hc in enumerate(hand):
                    if not hc.is_spell and hc.magnetic:
                        res = self._try_macro_chain(
                            g,
                            path,
                            (seaglass.TavernAction.activate(bi), seaglass.TavernAction.play(hi, bi)),
                        )
                        if res is not None:
                            results.append(res)
                        break

        # Macro 3: `Victorious Geomant` Overflow -> put `Geomagus Roogug` / `Gatekeeper Amalgam` left-most!
        if len(hand) >= 6 and not any(u.card_id == self._surveyor_id for u in board):
            for bi, bu in enumerate(board):
                if bu.card_id == self._geomant_id and not bu.activated_this_turn and obs.gold >= 2:
                    for ti, tu in enumerate(board):
                        if ti != 0 and tu.card_id in (self._roogug_id, self._amalgam_id):
                            new_bi = bi + 1 if bi < ti else bi
                            res = self._try_macro_chain(
                                g,
                                path,
                                (seaglass.TavernAction.reposition(ti, 0), seaglass.TavernAction.activate(new_bi)),
                            )
                            if res is not None:
                                results.append(res)
                            break

        # Macro 4: `Lurking Lionfish` / `Snarky Shark` -> move best Rally Beast to slot 0 first!
        best_rally_beast = None
        for i, u in enumerate(board):
            if ("beast" in u.tribes or "all" in u.tribes) and u.card_id in self._rally_ids:
                best_rally_beast = i
                break
        if best_rally_beast is not None and best_rally_beast != 0:
            for bi, bu in enumerate(board):
                if bu.card_id == self._lionfish_id and not bu.activated_this_turn and obs.gold >= 2 and shop:
                    new_bi = bi + 1 if bi < best_rally_beast else bi
                    res = self._try_macro_chain(
                        g,
                        path,
                        (
                            seaglass.TavernAction.reposition(best_rally_beast, 0),
                            seaglass.TavernAction.activate(new_bi, 0),
                        ),
                    )
                    if res is not None:
                        results.append(res)
                    break

        # Macro 5: `Greedy Conniver` Golden -> Sell for Tier 7 Discover!
        for bi, bu in enumerate(board):
            if bu.card_id == self._conniver_id and not bu.is_golden:
                for hi, hc in enumerate(hand):
                    if (not hc.is_spell and hc.card_id == self._elite_nav_id) or (
                        hc.is_spell and hc.name == "Eyes of the Earth Mother"
                    ):
                        res = self._try_macro_chain(
                            g,
                            path,
                            (seaglass.TavernAction.play(hi, bi), seaglass.TavernAction.sell(bi)),
                        )
                        if res is not None:
                            results.append(res)
                        break

        # Macro 6: `Living Prison` Activate(1) -> Buy biggest shop minion!
        if obs.gold >= 1 + obs.minion_cost and shop:
            for bi, bu in enumerate(board):
                if bu.card_id == self._living_prison_id and not bu.activated_this_turn:
                    best_si = None
                    best_st = 0
                    for si, su in enumerate(shop):
                        if not su.is_spell and (su.attack + su.health) > best_st:
                            best_st = su.attack + su.health
                            best_si = si
                    if best_si is not None:
                        res = self._try_macro_chain(
                            g,
                            path,
                            (seaglass.TavernAction.activate(bi), seaglass.TavernAction.buy(best_si)),
                        )
                        if res is not None:
                            results.append(res)
                    break

        # Macro 7: `Cataclysmic Harbinger` -> cast filler spell FIRST, payoff spell LAST!
        if any(u.card_id == self._harbinger_id for u in board) and len(hand) >= 2:
            filler_idx = None
            payoff_idx = None
            for hi, hc in enumerate(hand):
                if hc.is_spell and not hc.unplayable:
                    if hc.name in _HARBINGER_PAYOFF_SPELLS:
                        payoff_idx = hi
                    elif hc.name != "Blood Gem" and filler_idx is None:
                        filler_idx = hi
            if filler_idx is not None and payoff_idx is not None:
                adj_payoff = payoff_idx - 1 if payoff_idx > filler_idx else payoff_idx
                res = self._try_macro_chain(
                    g,
                    path,
                    (
                        seaglass.TavernAction.play(filler_idx, 0),
                        seaglass.TavernAction.play(adj_payoff, 0),
                    ),
                )
                if res is not None:
                    results.append(res)

        # Macro 8: `Arcane Behemoth` -> Sell Elemental or Buy+Play+Sell Elemental feeder!
        if any(u.card_id == self._behemoth_id for u in board) and obs.gold >= obs.minion_cost and n_board < 7:
            for si, su in enumerate(shop):
                if not su.is_spell and ("elemental" in su.tribes or "all" in su.tribes):
                    nhi = len(hand)
                    res = self._try_macro_chain(
                        g,
                        path,
                        (
                            seaglass.TavernAction.buy(si),
                            seaglass.TavernAction.play(nhi, n_board),
                            seaglass.TavernAction.sell(n_board),
                        ),
                    )
                    if res is not None:
                        results.append(res)
                    break

        # Macro 9: Top-3 / Finals Scam Swap -> `Buy(scam) + Sell(dead_scaler) + Play(scam)`!
        alive_units = ctx.alive_units if ctx is not None else 8
        if alive_units <= 3 and obs.turn >= 10 and n_board >= 7 and obs.gold >= obs.minion_cost:
            dead_slot = None
            worst_sc = 1e18
            for bi, bu in enumerate(board):
                if bu.venomous or bu.card_id in self.reg.scam_ids:
                    continue
                sc = (bu.attack + bu.health) * (0.25 if bu.card_id in self.arch_eval._finals_dead_scaler_ids else 1.0)
                if bu.divine_shield:
                    sc *= 1.6
                if sc < worst_sc:
                    worst_sc = sc
                    dead_slot = bi
            if dead_slot is not None:
                nhi = len(hand)
                for si, su in enumerate(shop):
                    if not su.is_spell and (su.venomous or su.card_id in self.reg.scam_ids):
                        res = self._try_macro_chain(
                            g,
                            path,
                            (
                                seaglass.TavernAction.buy(si),
                                seaglass.TavernAction.sell(dead_slot),
                                seaglass.TavernAction.play(nhi, 6),
                            ),
                        )
                        if res is not None:
                            results.append(res)

        return results
