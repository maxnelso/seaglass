"""Turn-End Evaluator: 1-Turn Rust Physics Rollout + Tournament ICM + Set-Transformer Synergy Value."""

from __future__ import annotations

import math
import os
from typing import Dict, List, Optional, Sequence

import seaglass

from .belief import OpponentParticleSet
from .catalog_meta import CardMetaDB, get_meta_db
from .value_model import SynergyValuePredictor, get_value_predictor

_TIER_CURVE_TABLE = (0.0, 0.0, 5.5, 13.0, 23.5, 35.5, 47.0, 55.0)
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


class LeafEvaluator:
    """
    Scores Tavern states by combining:
      1. 1-Turn Rust Engine Physics Rollouts (`end_turn` -> `resolve_combat_against` -> `start_turn`),
      2. Exact Monte Carlo Combat Rollouts + Poker-ICM Tournament Survival Utility,
      3. Learned Set-Transformer Placement Value Model (`SynergyValueNet`) + Structural Prior.
    """

    def __init__(
        self,
        catalog_name: str = "full",
        use_physics_rollout: bool = True,
        use_neural_value: bool = True,
        neural_weight: float = 0.75,
        checkpoint_path: Optional[os.PathLike | str] = None,
        legacy_mode: bool = False,
        explore_tribe: Optional[str] = None,
        explore_tier_bias: float = 0.0,
    ) -> None:
        self.catalog_name = catalog_name
        self.meta: CardMetaDB = get_meta_db(catalog_name)
        self.legacy_mode = legacy_mode
        self.use_physics_rollout = (not legacy_mode) and use_physics_rollout
        self.use_neural_value = (not legacy_mode) and use_neural_value
        self.neural_weight = neural_weight
        self.explore_tribe = explore_tribe
        self.explore_tier_bias = explore_tier_bias
        self.predictor: Optional[SynergyValuePredictor] = (
            get_value_predictor(catalog_name=catalog_name, checkpoint_path=checkpoint_path)
            if self.use_neural_value
            else None
        )

    @property
    def has_neural_model(self) -> bool:
        return self.predictor is not None and self.predictor.is_ready

    @staticmethod
    def icm_survival_utility(
        hp_after: float,
        alive_units: int,
        lobby_hps: Sequence[int],
    ) -> float:
        """Concave tournament survival utility inspired by Poker ICM."""
        if hp_after <= 0.0:
            return -65.0 - 8.5 * max(0, alive_units - 2)

        base = 26.0 * math.log1p(hp_after / 7.5)
        if lobby_hps:
            median_hp = sorted(lobby_hps)[len(lobby_hps) // 2]
            rel = (hp_after - median_hp) / max(10.0, float(median_hp))
            base += 4.5 * math.tanh(rel)
        return base

    def unit_static_value(
        self,
        u: seaglass.Unit,
        turn: int,
        tribe_counts: Dict[str, int],
    ) -> float:
        """Intrinsic + tribe/engine value of a single minion on board."""
        atk = float(max(0, u.attack))
        hp = float(max(0, u.health))
        val = 0.48 * atk + 0.46 * hp + 0.90 * u.tavern_tier

        if u.divine_shield:
            val += 2.6 + 0.46 * atk
        if u.reborn:
            val += 1.7 + 0.22 * u.tavern_tier
        if u.venomous:
            val += 5.5 + 0.35 * min(turn, 14) + (4.5 if u.divine_shield else 0.0)
        if u.windfury:
            val += 0.8 + 0.16 * atk
        if u.taunt:
            val += 0.6 + 0.05 * hp
        if u.is_golden:
            val += 3.0 + 0.65 * u.tavern_tier

        val += 1.8 * u.end_of_turn_stats
        val += 1.5 * u.extra_deathrattles
        val += 1.0 * len(u.dark_gifts)

        prof = self.meta.profile(u.card_id)
        if prof is not None:
            horizon = max(0.25, min(1.35, (15.5 - turn) / 9.0))
            val += prof.engine_weight * horizon * (1.65 if u.is_golden else 1.0)
            if prof.has_cleave:
                val += 0.45 * atk
            # Penalize stale Tier-1/2 vanilla bodies late game so the bot replaces them!
            if turn >= 7 and u.tavern_tier <= 2 and not prof.is_scaling_engine and (atk + hp) < 18:
                val -= 0.45 * (turn - 6)
            for tr in prof.tribes:
                if self.explore_tribe is not None and tr == self.explore_tribe:
                    val += 1.8
                cnt = tribe_counts.get(tr, 0)
                if cnt >= 2 and not self.legacy_mode:
                    val += 0.45 * min(cnt - 1, 5)
                elif cnt >= 2 and self.legacy_mode:
                    val += 0.65 * min(cnt - 1, 5)
        return val

    def _legacy_scaling_and_economy_value(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = True,
    ) -> float:
        """Exact v1 heuristic kept only for head-to-head regression benchmarking."""
        _ = ctx
        turn = max(1, obs.turn)
        hp_total = max(1, obs.health + obs.armor)
        safety = min(1.0, max(0.25, hp_total / 22.0))
        remaining_turns = max(1.0, 15.0 - turn)

        tribe_counts: Dict[str, int] = {}
        copy_counts: Dict[int, int] = {}
        has_summoner = False
        has_avenge_or_obs = False

        for u in obs.board:
            for tr in u.tribes:
                tribe_counts[tr] = tribe_counts.get(tr, 0) + 1
            if not u.is_golden and not u.is_spell:
                copy_counts[u.card_id] = copy_counts.get(u.card_id, 0) + 1
            prof = self.meta.profile(u.card_id)
            if prof:
                has_summoner = has_summoner or prof.is_combat_summoner
                has_avenge_or_obs = (
                    has_avenge_or_obs or prof.has_avenge or prof.is_combat_observer
                )

        score = sum(self.unit_static_value(u, turn, tribe_counts) for u in obs.board)
        if has_summoner and has_avenge_or_obs:
            score += 4.0 * safety
        tier = obs.tavern_tier
        score += (tier - 1) * (2.6 + 0.55 * remaining_turns) * safety

        for c in obs.hand:
            if not c.is_golden and not c.is_spell:
                copy_counts[c.card_id] = copy_counts.get(c.card_id, 0) + 1
            if c.is_golden:
                score += 6.5 * safety
            elif c.is_spell:
                score += -0.5 if end_of_turn else 2.2
            else:
                if end_of_turn and len(obs.board) < 7 and not c.unplayable:
                    score -= 6.0
                else:
                    score += (1.2 + 0.4 * c.tavern_tier) * safety

        for cid, cnt in copy_counts.items():
            if cnt == 2:
                tmpl = self.meta.templates_by_id.get(cid)
                t_val = tmpl.tavern_tier if tmpl else 1
                score += (2.0 + 0.45 * min(6, tier + 1) + 0.2 * t_val) * safety

        if not end_of_turn:
            score += 1.45 * obs.gold + 0.85 * obs.free_refreshes
        else:
            score -= 0.35 * obs.gold
        return score

    def scaling_and_economy_value(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = True,
    ) -> float:
        """
        Bug-free structural macro prior:
          - Superlinear archetype commitment + multi-unit engine x fuel multipliers,
          - Realistic Tavern Tier 4-6 curve & upgrade discount valuation,
          - Full visibility into all 54 engine player_counters, Deity, card_counters, and Trinkets,
          - Eliminates hand-hoarding, Golden-in-hand hoarding, and junk end-of-turn purchases.
        """
        if self.legacy_mode:
            return self._legacy_scaling_and_economy_value(obs, ctx=ctx, end_of_turn=end_of_turn)

        turn = max(1, obs.turn)
        hp_total = max(1, obs.health + obs.armor)
        safety = min(1.0, max(0.25, hp_total / 20.0))
        horizon = max(0.25, min(1.35, (15.5 - turn) / 9.0))

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

        for u in obs.board:
            for tr in u.tribes:
                tribe_counts[tr] = tribe_counts.get(tr, 0) + 1
            if not u.is_golden and not u.is_spell:
                board_copy_counts[u.card_id] = board_copy_counts.get(u.card_id, 0) + 1
            prof = self.meta.profile(u.card_id)
            if prof is not None:
                mult = 2 if u.is_golden else 1
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

        board_unit_scores = [self.unit_static_value(u, turn, tribe_counts) for u in obs.board]
        score = sum(board_unit_scores)
        # Explicit board-presence tempo per filled slot (prevents mean-pool dilution / selling down below 7).
        early_slot_w = 2.6 if turn <= 6 else (1.8 if turn <= 10 else 1.2)
        score += early_slot_w * min(7, len(obs.board))
        weakest_board_score = min(board_unit_scores) if board_unit_scores else 0.0

        # 1. Superlinear single-archetype commitment (rewards 4-6 coherent units over rainbow soup).
        if tribe_counts:
            dom_cnt = max(tribe_counts.values())
            if dom_cnt >= 3:
                score += 1.15 * ((dom_cnt - 2) ** 1.45) * safety

        # 2. Multi-unit multiplicative combo bonuses across board hooks.
        if summoners > 0 and combat_obs > 0:
            score += 2.4 * min(4, summoners) * min(4, combat_obs) * safety
        if eot_engines >= 2:
            score += 1.8 * (eot_engines - 1) * horizon * safety
        if bc_engines > 0 and battlecries > 0:
            score += 2.2 * bc_engines * battlecries * horizon * safety
        if mag_engines > 0 and tribe_counts.get("mech", 0) >= 2:
            score += 2.5 * mag_engines * min(5, tribe_counts.get("mech", 0)) * horizon * safety
        if play_engines > 0:
            score += 1.6 * play_engines * horizon * safety
        if gold_engines > 0:
            score += 1.7 * gold_engines * horizon * safety

        # 3. Tavern Tier curve + upgrade_cost progress (unlocks Tier 4-6 build-defining engines!).
        tier = max(1, min(7, obs.tavern_tier))
        tier_base = _TIER_CURVE_TABLE[tier] + self.explore_tier_bias * (tier - 1)
        score += tier_base * horizon * safety
        if tier < 6:
            # Credit natural & effect-driven upgrade cost reductions so upgrading cleanly is valued.
            nominal_upg = tier + 5
            discount = max(0, nominal_upg - int(obs.upgrade_cost))
            score += 0.45 * discount * horizon * safety

        # 4. Hand discipline: play merged Goldens & upgrades immediately; never clog hand!
        all_copy_counts = dict(board_copy_counts)
        playable_hand_minions = 0
        for c in obs.hand:
            prof = self.meta.profile(c.card_id)
            is_intrinsic_gold = prof.is_intrinsic_golden if prof is not None else False
            if not c.is_golden and not c.is_spell:
                all_copy_counts[c.card_id] = all_copy_counts.get(c.card_id, 0) + 1

            if c.is_golden and not c.is_spell and not is_intrinsic_gold:
                if end_of_turn and not c.unplayable:
                    # Leaving a merged Golden minion in hand forfeits its Triple Reward Discover!
                    score -= 4.5
                else:
                    score += 7.5 * safety
            elif c.is_spell:
                is_temp = prof.is_temporary if prof is not None else False
                if end_of_turn:
                    score += -1.5 if is_temp else -0.35
                else:
                    # Triple Rewards & Discover spells in hand are high-priority to cast!
                    score += 3.2 if c.spell_cost == 0 else 1.6
            else:
                if c.unplayable:
                    # Locked / Lockbox minion ticking down for a future turn.
                    score += (1.8 + 0.5 * c.tavern_tier) * safety
                elif end_of_turn:
                    playable_hand_minions += 1
                    if len(obs.board) < 7:
                        score -= 8.5
                    else:
                        cand_val = self.unit_static_value(c, turn, tribe_counts)
                        if cand_val > weakest_board_score + 4.0:
                            # Penalize leaving a strictly stronger minion in hand over a weak slot!
                            score -= 2.5
                        elif playable_hand_minions == 1:
                            score += 0.25 * safety
                        else:
                            # Moderate penalty for holding extra plain minions (never outweighs a big board unit!).
                            score -= 0.9
                else:
                    # Interior search node: only credit hand minions if board has space or a small cycle slot.
                    if len(obs.board) < 7:
                        score += (1.8 + 0.45 * c.tavern_tier) * safety
                    else:
                        score += (0.55 + 0.25 * c.tavern_tier) * safety

        # 5. Pair potential: reward pairs on board (plus at most 1 held pair piece, only for Tier >= tier-1).
        pairs_counted = 0
        for cid, cnt in all_copy_counts.items():
            if cnt == 2 and pairs_counted < 2:
                tmpl = self.meta.templates_by_id.get(cid)
                t_val = tmpl.tavern_tier if tmpl else 1
                if turn <= 6 or t_val >= max(2, tier - 1) or board_copy_counts.get(cid, 0) == 2:
                    pairs_counted += 1
                    score += (1.3 + 0.35 * min(6, tier + 1) + 0.25 * t_val) * safety

        # 6. Frozen shop evaluation: ONLY freeze when out of gold for a relevant Triple or high-tier engine!
        if obs.is_frozen:
            score += self._freeze_shop_value(obs, all_copy_counts) * safety

        # 7. Full visibility into persistent engine counters, Deity, card_counters, and Trinkets!
        score += 1.65 * obs.bonus_gold_next_turn * safety
        score += 2.8 * self.meta.counter(obs, "base_max_gold_bonus") * horizon * safety

        gem_atk = self.meta.counter(obs, "blood_gem_bonus_atk")
        gem_hp = self.meta.counter(obs, "blood_gem_bonus_hp")
        qb_mult = 1.0 + 0.35 * tribe_counts.get("quilboar", 0)
        score += 0.95 * (gem_atk + gem_hp) * qb_mult * safety

        sp_atk = self.meta.counter(obs, "spell_bonus_atk")
        sp_hp = self.meta.counter(obs, "spell_bonus_hp")
        score += (0.85 + 0.35 * spell_engines) * (sp_atk + sp_hp) * safety

        el_atk = self.meta.counter(obs, "tavern_elemental_atk")
        el_hp = self.meta.counter(obs, "tavern_elemental_hp")
        el_mult = 0.7 + 0.35 * tribe_counts.get("elemental", 0)
        score += 0.75 * (el_atk + el_hp) * el_mult * safety

        all_atk = self.meta.counter(obs, "tavern_all_atk")
        all_hp = self.meta.counter(obs, "tavern_all_hp")
        score += 0.90 * (all_atk + all_hp) * safety

        ud_atk = self.meta.counter(obs, "undead_bonus_attack")
        ud_hp = self.meta.counter(obs, "undead_bonus_health")
        ud_mult = 0.6 + 0.45 * tribe_counts.get("undead", 0)
        score += 0.85 * (ud_atk + ud_hp) * ud_mult * safety

        for tr_name, c_atk, c_hp in _TRIBE_TAVERN_COUNTER_PAIRS:
            tb = self.meta.counter(obs, c_atk) + self.meta.counter(obs, c_hp)
            if tb > 0:
                score += 0.55 * tb * (0.6 + 0.35 * tribe_counts.get(tr_name, 0)) * safety

        score += 1.1 * self.meta.counter(obs, "refresh_blood_gems") * qb_mult * safety
        score += 0.7 * self.meta.counter(obs, "refresh_buffs") * safety

        if obs.deity != "none" and tribe_counts.get("aberration", 0) >= 1:
            deity_stats = max(0, obs.deity_attack - 1) + max(0, obs.deity_health - 1)
            ab_mult = min(1.3, 0.30 * tribe_counts.get("aberration", 0))
            score += 0.48 * deity_stats * ab_mult

        for _, v1, v2 in obs.card_counters:
            score += 0.55 * (max(0, v1) + max(0, v2)) * safety

        score += 2.5 * len(obs.trinkets) * safety

        # 8. Purchasing power on interior beam nodes only (NO penalty for floating 1g at EndTurn!).
        if not end_of_turn:
            score += 1.55 * obs.gold + 0.90 * obs.free_refreshes + 0.65 * max(0, obs.next_minion_discount)
            if obs.discover_pending:
                score += 6.5 + 1.0 * tier

        return score

    def evaluate_fast_batch(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = False,
    ) -> List[float]:
        """Batched evaluation for interior DAG beam pruning (neural + structural anchor)."""
        if not obs_list:
            return []
        prior_scores = [
            self.scaling_and_economy_value(o, ctx=ctx, end_of_turn=end_of_turn)
            for o in obs_list
        ]
        if not self.has_neural_model or self.predictor is None:
            return prior_scores

        net_scores = self.predictor.predict_batch(obs_list, ctx=ctx, end_of_turn=end_of_turn)
        w = self.neural_weight
        out: List[float] = []
        for o, p_val, n_val in zip(obs_list, prior_scores, net_scores):
            # Scale neural placement utility (~[-26, +30]) to match structural macro scale (~1.6x)
            # and keep unspent-gold / open-slot / tier-curve micro-discipline crisp.
            turn_o = max(1, o.turn)
            hp_o = max(1, o.health + o.armor)
            safety_o = min(1.0, max(0.25, hp_o / 20.0))
            horizon_o = max(0.25, min(1.35, (15.5 - turn_o) / 9.0))
            tier_o = max(1, min(7, o.tavern_tier))
            slot_w_o = 2.4 if turn_o <= 6 else (1.5 if turn_o <= 10 else 0.9)
            micro_guard = (
                0.55 * _TIER_CURVE_TABLE[tier_o] * horizon_o * safety_o
                + slot_w_o * min(7, len(o.board))
            )
            if not end_of_turn:
                micro_guard += 1.45 * o.gold + 0.85 * o.free_refreshes
                if o.discover_pending:
                    micro_guard += 6.0 + 0.9 * o.tavern_tier
            else:
                playable_plain = 0
                for c in o.hand:
                    c_prof = self.meta.profile(c.card_id)
                    is_intr_g = c_prof.is_intrinsic_golden if c_prof is not None else False
                    if c.is_golden and not c.is_spell and not c.unplayable and not is_intr_g:
                        micro_guard -= 4.5
                    elif not c.is_spell and not c.unplayable:
                        playable_plain += 1
                        if len(o.board) < 7:
                            micro_guard -= 8.0
                        elif playable_plain >= 2:
                            micro_guard -= 0.8
            blended = (1.0 - w) * p_val + w * (1.65 * n_val + micro_guard)
            out.append(blended)
        return out

    def evaluate_fast(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = False,
    ) -> float:
        """Single-state fast evaluation."""
        return self.evaluate_fast_batch([obs], ctx=ctx, end_of_turn=end_of_turn)[0]

    def evaluate_turn_end(
        self,
        tavern: seaglass.TavernGame,
        particles: OpponentParticleSet,
        ctx: Optional[seaglass.LobbyContext] = None,
        combats_per_particle: int = 16,
        board_override: Optional[List[seaglass.Unit]] = None,
    ) -> float:
        """
        Full Evaluation:
          1. Step `end_turn()` on a clone so ALL Rust End-of-Turn combos execute before combat.
          2. Run Monte Carlo `equity_against` on the post-`EndTurn` warband + ICM survival curve.
          3. Step `resolve_combat_against` + `start_turn()` (1-Turn Engine Physics Rollout) so
             Combat-to-Tavern persistence (`Poet`, `Tarecgosa`, `Rylak`, `Avenge`) & `TurnStart`
             effects materialize into Turn `t+1`, then score Turn `t+1` with `SynergyValueNet`.
        """
        obs_pre = tavern.observe()
        hp_now = float(obs_pre.health + obs_pre.armor)
        alive_units = ctx.alive_units if ctx is not None else 8
        damage_cap = ctx.damage_cap if ctx is not None else None
        is_ghost = ctx.next_is_ghost if ctx is not None else False
        lobby_hps = (
            [o.health + o.armor for o in ctx.opponents if o.alive]
            if ctx is not None
            else [30] * 7
        )

        # Step 1: Execute `end_turn()` in Rust when enabled (unless overriding raw board order).
        if self.use_physics_rollout and board_override is None:
            combat_game = tavern.copy()
            combat_game.reseed((obs_pre.turn * 65537 + 17) | 1)
            if combat_game.is_legal(seaglass.TavernAction.end_turn()):
                combat_game.end_turn()
        else:
            combat_game = tavern

        # Step 2: Exact Monte Carlo combat rollouts + ICM tournament survival utility.
        icm_sum = 0.0
        offense_bonus_sum = 0.0
        lethal_prob_sum = 0.0
        n_boards = max(1, len(particles.boards))

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
                    max(float(particles.tier + 1), 0.62 * worst_loss),
                )
                if damage_cap is not None:
                    avg_loss = min(avg_loss, float(damage_cap))
            else:
                worst_loss = 0.0
                avg_loss = 0.0

            if hp_now - avg_loss <= 0.0:
                lethal_prob_sum += dist.b_win_rate
            elif hp_now - worst_loss <= 0.0:
                lethal_prob_sum += 0.35 * dist.b_win_rate

            u_win_or_draw = self.icm_survival_utility(hp_now, alive_units, lobby_hps)
            u_avg_loss = self.icm_survival_utility(
                hp_now - avg_loss, alive_units, lobby_hps
            )
            u_worst_loss = self.icm_survival_utility(
                hp_now - worst_loss, alive_units, lobby_hps
            )
            loss_utility = 0.72 * u_avg_loss + 0.28 * u_worst_loss

            expected_icm = (
                (dist.a_win_rate + dist.draw_rate) * u_win_or_draw
                + dist.b_win_rate * loss_utility
            )
            icm_sum += expected_icm

            if not is_ghost and dist.a_win_rate > 0.0:
                dealt = max(0.0, dist.damage_mean)
                if damage_cap is not None:
                    dealt = min(dealt, float(damage_cap))
                offense_bonus_sum += 0.22 * dealt + 2.2 * dist.a_win_rate

        combat_icm_score = (icm_sum + offense_bonus_sum) / n_boards
        survival_prob = max(0.0, min(1.0, 1.0 - (lethal_prob_sum / n_boards)))

        # Step 3: 1-Turn Engine Physics Rollout -> Turn `t + 1` Start-of-Turn Macro Evaluation!
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

            # Evaluate Turn `t + 1` post-`start_turn()` state (`end_of_turn=False` so unlocked
            # Lockboxes, TurnStart Spellcrafts, TurnStart gold, and banked Coins are valued as assets!)
            next_scores = self.evaluate_fast_batch(
                next_obs_samples, ctx=ctx, end_of_turn=False
            )
            avg_next_macro = sum(next_scores) / len(next_scores)

            # Preserve pre-EndTurn penalties for leaving playable minions/Goldens sitting in hand
            # AND pre-EndTurn freeze value (since `start_turn` consumes `is_frozen`).
            pre_discipline = self._pre_end_turn_discipline(obs_pre)
            continuation_weight = 0.25 + 0.75 * survival_prob
            macro_score = continuation_weight * avg_next_macro + pre_discipline
        else:
            macro_score = self.evaluate_fast(obs_pre, ctx=ctx, end_of_turn=True)

        return combat_icm_score + macro_score

    def _freeze_shop_value(
        self,
        obs: seaglass.TavernObservation,
        all_copies: Dict[int, int],
    ) -> float:
        """
        Strict freeze discipline:
          - Never reward freezing if the bot still had enough gold to buy the minion right now (`gold >= minion_cost`).
          - Never eternally freeze an obsolete low-tier pair (`tier - s.tier >= 3`) unless invested stats >= 20.
          - Keep freeze bridge bonus (`<= +2.3`) strictly smaller than actually buying the Triple next turn!
        """
        if obs.gold >= obs.minion_cost:
            return -1.8
        tier = obs.tavern_tier
        turn = obs.turn
        best_fz = -1.75
        for s in obs.shop:
            if s.is_spell:
                continue
            if not s.is_golden and all_copies.get(s.card_id, 0) >= 2:
                invested_stats = sum(
                    u.attack + u.health for u in obs.board if u.card_id == s.card_id
                )
                if s.tavern_tier >= max(1, tier - 2) or turn <= 7 or invested_stats >= 22:
                    best_fz = max(best_fz, 2.25)
            else:
                prof = self.meta.profile(s.card_id)
                if prof and prof.is_scaling_engine and s.tavern_tier >= max(4, tier):
                    best_fz = max(best_fz, 1.65)
                elif s.tavern_tier >= max(4, tier + 1):
                    best_fz = max(best_fz, 1.1)
        return best_fz

    def _pre_end_turn_discipline(self, obs: seaglass.TavernObservation) -> float:
        """Scores pre-EndTurn invariants that `start_turn()` clears (`is_frozen`, unplayed hand cards)."""
        adj = 0.0
        hp_total = max(1, obs.health + obs.armor)
        safety = min(1.0, max(0.25, hp_total / 20.0))

        all_copies: Dict[int, int] = {}
        for u in list(obs.board) + list(obs.hand):
            if not u.is_golden and not u.is_spell:
                all_copies[u.card_id] = all_copies.get(u.card_id, 0) + 1

        playable_minions = 0
        for c in obs.hand:
            c_prof = self.meta.profile(c.card_id)
            is_intr_g = c_prof.is_intrinsic_golden if c_prof is not None else False
            if c.is_golden and not c.is_spell and not c.unplayable and not is_intr_g:
                adj -= 9.5
            elif c.is_spell:
                is_temp = c_prof.is_temporary if c_prof is not None else False
                if is_temp:
                    adj -= 2.5
            elif not c.unplayable:
                playable_minions += 1
                if len(obs.board) < 7:
                    adj -= 12.0
                elif playable_minions == 1:
                    adj -= 1.2
                else:
                    adj -= 2.8

        if obs.is_frozen:
            adj += self._freeze_shop_value(obs, all_copies) * safety

        return adj
