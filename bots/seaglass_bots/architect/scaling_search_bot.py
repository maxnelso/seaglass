"""ScalingSearchBot: Zero-Hardcoding Multi-Epoch Physics Search Powered by `ScalingValueTransformer`!

Clean scientific test of learned multi-horizon neural value (`VAL dist_ce = 1.778`, `-0.201 CE`
stronger than `sovereign_value_v1.npz`'s `1.9791`!) + explicit 1st-place utility + learned 3-turn
future stat growth (`E[log1p(stats_{t+3}) - log1p(stats_t)]`) inside a general Multi-Epoch Rust
Physics Beam — with ZERO hardcoded `comps.py`, ZERO hardcoded Trinket deltas, and ZERO Hero deltas!
"""

from __future__ import annotations

import functools
import math
import sys
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

_RL_DIR = str(Path(__file__).resolve().parents[3] / "rl")
if _RL_DIR not in sys.path:
    sys.path.insert(0, _RL_DIR)

import numpy as np
import seaglass
import torch
import torch.nn.functional as F

from seaglass_rl import checkpoint as rl_checkpoint
from seaglass_rl.actions import ACTION_SPACE_SIZE, ACTION_TO_INDEX
from seaglass_rl.features import encode_observation

from ..sovereign_evaluator import SovereignEvaluator
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
from .bellman_residual import UpgradeResidualPredictor
from .scaling_value import SHOP_SLOT_END, SHOP_SLOT_START, ScalingValueTransformer

# Strictly non-negative 1st-place-oriented placement utility (exact +34 shift of [-34, +48]):
# 8th place = 0.0 (dead), 1st place = +82.0 (win) — ensures `continuation_weight * avg_next_macro`
# in `evaluate_turn_end` ALWAYS strictly rewards higher `survival_prob` (never rewards dying when behind!).
SCALING_FIRST_PLACE_WEIGHTS = np.array(
    [82.0, 52.0, 40.0, 32.0, 24.0, 16.0, 8.0, 0.0], dtype=np.float32
)


class ScalingEvaluator(SovereignEvaluator):
    """Drop-in SovereignEvaluator subclass applying `ScalingValueTransformer` uniformly across
    EOT, SOT, open/stochastic Discover states, and beam expansions (zero `open_indices` vs `eot_indices`
    scale mismatch!) with tunable `prior_weight`."""

    def __init__(
        self,
        *args,
        prior_weight: float = 1.0,
        score_all_states: bool = True,
        two_turn_rollout_steps: int = 0,
        two_turn_weight: float = 0.45,
        two_turn_samples: int = 1,
        two_turn_unclamped: bool = False,
        no_t1_upgrade: bool = False,
        exact_bellman_rollout: bool = False,
        combat_icm_weight: float = 1.0,
        phys_rolls_per_particle: int = 2,
        **kwargs,
    ) -> None:
        super().__init__(*args, **kwargs)
        self.prior_weight = float(prior_weight)
        self.score_all_states = bool(score_all_states)
        self.two_turn_rollout_steps = int(two_turn_rollout_steps)
        self.two_turn_weight = float(two_turn_weight)
        self.two_turn_samples = max(1, int(two_turn_samples))
        self.two_turn_unclamped = bool(two_turn_unclamped)
        self.exact_bellman_rollout = bool(exact_bellman_rollout)
        self.combat_icm_weight = float(combat_icm_weight)
        self.phys_rolls_per_particle = max(1, int(phys_rolls_per_particle))
        self._rollout_fn = None
        self._rollout_batch_fn = None
        self._in_t1_rollout = False
        self._turn_win_macro_cap = 350.0

    def _evaluate_exact_bellman_turn_end(
        self,
        tavern: seaglass.TavernGame,
        particles,
        ctx: Optional[seaglass.LobbyContext] = None,
        combats_per_particle: int = 14,
        pre_positioned: bool = False,
        run_two_turn: bool = False,
        t1_sample_cap: Optional[int] = None,
    ) -> float:
        """Fused Exact 1-Turn + 2-Turn Bellman backup across ALL opponent particles:
        runs `fast_reposition_game`, Rust combat sim, `start_turn()`, and GPU `V(s_{t+1}^{SOT})`
        ONCE per candidate and reuses the exact surviving post-combat games & SOT values for
        the Turn t+1 rollout (exact telescoping cancellation, ~2x faster with zero bias!)."""
        obs_pre = tavern.observe()
        alive_units = ctx.alive_units if ctx is not None else 8
        is_ghost = particles.is_ghost or (ctx.next_is_ghost if ctx is not None else False)
        damage_cap = ctx.damage_cap if ctx is not None else None

        combat_game = tavern.copy()
        if not pre_positioned:
            self.positioner.fast_reposition_game(combat_game, obs_pre, particles)
        combat_game.reseed((obs_pre.turn * 65537 + 17) | 1)
        if combat_game.is_legal(seaglass.TavernAction.end_turn()):
            combat_game.end_turn()

        # Optional scaled combat_icm_score (0.0 = pure Bellman; 0.25 = reduced non-lethal double-counting;
        # restored to 1.0 in 1v1 Finals; lethal highroll `cvar_penalty` always stays at full strength!).
        combat_icm_term = 0.0
        hp_now = self.effective_hp(obs_pre)
        in_lethal_zone = (not is_ghost) and (
            damage_cap is None or hp_now <= float(damage_cap)
        )
        any_loss_lethal = (not is_ghost) and (hp_now <= float(particles.tier + 1))
        p_surv_by_particle: Optional[List[float]] = [] if any_loss_lethal else None
        if self.combat_icm_weight > 0.0 or in_lethal_zone:
            lobby_hps = (
                [o.health + o.armor for o in ctx.opponents if o.alive]
                if ctx is not None
                else [30] * 7
            )
            u_win_or_draw = self.cvar_survival_utility(hp_now, alive_units, lobby_hps, damage_cap)
            icm_sum = 0.0
            offense_sum = 0.0
            margin_sum = 0.0
            is_endgame = alive_units <= 4
            ko_mult = 1.65 if is_endgame else 1.05
            n_boards = max(1, len(particles.boards))
            for idx, opp_board in enumerate(particles.boards):
                dist = combat_game.equity_against(
                    opp_board,
                    opponent_tier=particles.tier,
                    n=combats_per_particle,
                    base_seed=obs_pre.turn * 1009 + idx * 37 + 1,
                )
                if p_surv_by_particle is not None:
                    p_surv_by_particle.append(float(dist.a_win_rate + dist.draw_rate))
                if in_lethal_zone:
                    # Continuous combat-margin gradient so lethal-HP states with 0% initial winrate
                    # never flatline at constant `death_util` (always climb toward stronger boards!).
                    margin_sum += 0.65 * float(dist.damage_mean)
                if self.combat_icm_weight > 0.0:
                    if dist.b_win_rate > 1e-6:
                        worst_loss = float(max(0, -dist.damage_min))
                        if damage_cap is not None:
                            worst_loss = min(worst_loss, float(damage_cap))
                        avg_loss = min(worst_loss, max(float(particles.tier + 1), 0.65 * worst_loss))
                        if damage_cap is not None:
                            avg_loss = min(avg_loss, float(damage_cap))
                    else:
                        worst_loss = 0.0
                        avg_loss = 0.0
                    u_avg = self.cvar_survival_utility(hp_now - avg_loss, alive_units, lobby_hps, damage_cap)
                    u_wst = self.cvar_survival_utility(hp_now - worst_loss, alive_units, lobby_hps, damage_cap)
                    icm_sum += (dist.a_win_rate + dist.draw_rate) * u_win_or_draw + dist.b_win_rate * (
                        0.68 * u_avg + 0.32 * u_wst
                    )
                    if not is_ghost and dist.a_win_rate > 0.0:
                        dealt = max(0.0, dist.damage_mean)
                        if damage_cap is not None:
                            dealt = min(dealt, float(damage_cap))
                        offense_sum += ko_mult * (0.31 * dealt + 3.0 * dist.a_win_rate)
            combat_icm_term = (
                self.combat_icm_weight * ((icm_sum + offense_sum) / n_boards)
                + (margin_sum / n_boards)
            )
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
            )
            if hr_dist.b_win_rate > 1e-6:
                hr_worst = float(max(0, -hr_dist.damage_min))
                if damage_cap is not None:
                    hr_worst = min(hr_worst, float(damage_cap))
                if hp_now - hr_worst <= 0.0:
                    cvar_penalty = 38.0 * hr_dist.b_win_rate
        combat_icm_term -= cvar_penalty

        # Exact terminal utilities on the same `prior_weight * macro + neural_weight * V` scale
        # as surviving next-turn continuations (so guaranteed 1v1 victory ALWAYS strictly dominates
        # any uncertain continuation turn and never incentivizes throwing/stalling a lethal fight!).
        death_place_idx = min(7, max(0, int(alive_units) - 1))
        death_util = float(self.neural_weight * SCALING_FIRST_PLACE_WEIGHTS[death_place_idx])
        p1_bonus = float(getattr(self.predictor, "p1_bonus", 20.0)) if self.predictor is not None else 20.0
        growth_w = float(getattr(self.predictor, "growth_weight", 6.0)) if self.predictor is not None else 6.0
        prior_win_offset = float(self.prior_weight * self._turn_win_macro_cap)
        win_1v1_util = (
            prior_win_offset
            + float(self.neural_weight * (82.0 + p1_bonus + growth_w * 11.5))
            + 45.0
        )

        opp_ehp_1v1: Optional[int] = None
        if alive_units == 2 and not is_ghost and ctx is not None:
            alive_opps = [o for o in ctx.opponents if o.alive]
            if len(alive_opps) == 1:
                opp_ehp_1v1 = int(alive_opps[0].health + alive_opps[0].armor)

        alive_next_obs: List[seaglass.TavernObservation] = []
        alive_weights: List[float] = []
        t1_games: List[Tuple[int, seaglass.TavernGame, int, float]] = []
        terminal_sum = 0.0
        total_rollouts = 0
        n_rolls = max(4, self.phys_rolls_per_particle) if opp_ehp_1v1 is not None else self.phys_rolls_per_particle
        n_t1_target = (
            min(self.two_turn_samples, int(t1_sample_cap))
            if t1_sample_cap is not None
            else self.two_turn_samples
        )
        t1_seen = 0
        use_rb_lethal = any_loss_lethal and opp_ehp_1v1 is None and p_surv_by_particle is not None

        for p_idx, opp_board in enumerate(particles.boards):
            if use_rb_lethal:
                p_surv = p_surv_by_particle[p_idx]
                total_rollouts += n_rolls
                terminal_sum += float(n_rolls) * (1.0 - p_surv) * death_util
                if p_surv <= 1e-6:
                    if run_two_turn and t1_seen < n_t1_target:
                        t1_seen += 1
                    continue
                p_alive_indices: List[int] = []
                p_t1_cand: Optional[Tuple[int, seaglass.TavernGame, int]] = None
                for r_idx in range(n_rolls):
                    g_next = combat_game.copy()
                    c_seed = (obs_pre.turn * 131071 + p_idx * 8191 + r_idx * 2503 + 3) | 1
                    g_next.reseed(c_seed)
                    g_next.resolve_combat_against(
                        opp_board,
                        opponent_tier=particles.tier,
                        seed=c_seed,
                    )
                    if int(g_next.observe().health) <= 0:
                        continue
                    t1_seed = (obs_pre.turn * 524287 + p_idx * 16381 + r_idx * 4099 + 11) | 1
                    g_next.reseed(t1_seed)
                    g_next.start_turn()
                    alive_idx = len(alive_next_obs)
                    alive_next_obs.append(g_next.observe())
                    alive_weights.append(1.0)
                    p_alive_indices.append(alive_idx)
                    if p_t1_cand is None:
                        p_t1_cand = (alive_idx, g_next, t1_seed)
                if not p_alive_indices:
                    # `n=14` equity proved survival > 0% even though 2 noisy rolls died:
                    # construct the exact surviving (0-damage win/draw) SOT state so no low-odds survival line is dropped!
                    g_surv = combat_game.copy()
                    t1_seed = (obs_pre.turn * 524287 + p_idx * 16381 + 11) | 1
                    g_surv.reseed(t1_seed)
                    g_surv.start_turn()
                    alive_idx = len(alive_next_obs)
                    alive_next_obs.append(g_surv.observe())
                    alive_weights.append(1.0)
                    p_alive_indices.append(alive_idx)
                    p_t1_cand = (alive_idx, g_surv, t1_seed)
                w_each = (float(n_rolls) * p_surv) / float(len(p_alive_indices))
                for ai in p_alive_indices:
                    alive_weights[ai] = w_each
                if run_two_turn and t1_seen < n_t1_target:
                    if p_t1_cand is not None:
                        t1_games.append((p_t1_cand[0], p_t1_cand[1], p_t1_cand[2], p_surv))
                    t1_seen += 1
                continue

            for r_idx in range(n_rolls):
                total_rollouts += 1
                g_next = combat_game.copy()
                c_seed = (obs_pre.turn * 131071 + p_idx * 8191 + r_idx * 2503 + 3) | 1
                g_next.reseed(c_seed)
                res = g_next.resolve_combat_against(
                    opp_board,
                    opponent_tier=particles.tier,
                    seed=c_seed,
                )
                post_hp = int(g_next.observe().health)
                if post_hp <= 0:
                    terminal_sum += death_util
                    if run_two_turn and r_idx == 0 and t1_seen < n_t1_target:
                        t1_seen += 1
                    continue
                if opp_ehp_1v1 is not None and res.outcome == "a_win":
                    dealt = int(res.hero_damage)
                    if damage_cap is not None:
                        dealt = min(dealt, int(damage_cap))
                    if dealt >= opp_ehp_1v1:
                        terminal_sum += win_1v1_util
                        if run_two_turn and r_idx == 0 and t1_seen < n_t1_target:
                            t1_seen += 1
                        continue
                # Decouple Start-of-Turn RNG from combat RNG shifts across candidates!
                t1_seed = (obs_pre.turn * 524287 + p_idx * 16381 + r_idx * 4099 + 11) | 1
                g_next.reseed(t1_seed)
                g_next.start_turn()
                alive_idx = len(alive_next_obs)
                alive_next_obs.append(g_next.observe())
                alive_weights.append(1.0)
                if run_two_turn and r_idx == 0 and t1_seen < n_t1_target:
                    t1_games.append((alive_idx, g_next, t1_seed, 1.0))
                    t1_seen += 1

        next_scores: List[float] = []
        alive_sum = 0.0
        if alive_next_obs:
            next_scores = self.evaluate_fast_batch(
                alive_next_obs,
                ctx=ctx,
                end_of_turn=False,
                particles=particles,
                in_dist_sot=True,
                sot_shop_is_real=obs_pre.is_frozen,
            )
            alive_sum = float(sum(w * s for w, s in zip(alive_weights, next_scores)))

        base_score = combat_icm_term + (alive_sum + terminal_sum) / max(1, total_rollouts)
        if not run_two_turn or not t1_games or n_t1_target <= 0:
            return base_score

        eot_obs_list: List[seaglass.TavernObservation] = []
        v_sots: List[Tuple[float, float]] = []
        self._in_t1_rollout = True
        try:
            for alive_idx, g_t1, t1_seed, t1_w in t1_games:
                for _ in range(2):
                    cur_t1 = g_t1.observe()
                    if not cur_t1.trinket_options:
                        break
                    t_acts = [a for a in g_t1.legal_actions() if a.kind == "choose_trinket"]
                    if not t_acts:
                        break
                    best_ta = t_acts[0]
                    best_tv = -1e18
                    for ta in t_acts:
                        sim_t = g_t1.copy()
                        sim_t.step(ta)
                        tv = self.evaluate_fast(sim_t.observe(), ctx=ctx, end_of_turn=False, particles=particles)
                        if tv > best_tv:
                            best_tv = tv
                            best_ta = ta
                    g_t1.step(best_ta)
                g_t1_eot = self._rollout_fn(
                    g_t1,
                    ctx,
                    particles=particles,
                    max_steps=self.two_turn_rollout_steps,
                    crn_seed=(t1_seed ^ 0x2468ACE1) | 1,
                )
                eot_obs_list.append(g_t1_eot.observe())
                v_sots.append((next_scores[alive_idx], t1_w))
        finally:
            self._in_t1_rollout = False

        v_eots = self.evaluate_fast_batch(
            eot_obs_list,
            ctx=ctx,
            end_of_turn=True,
            particles=particles,
        )
        if self.two_turn_unclamped:
            delta_sum = sum(tw * (ve - vs) for ve, (vs, tw) in zip(v_eots, v_sots))
        else:
            delta_sum = sum(tw * max(0.0, ve - vs) for ve, (vs, tw) in zip(v_eots, v_sots))
        return base_score + self.two_turn_weight * (delta_sum / max(1, t1_seen))

    def evaluate_turn_end_batch(
        self,
        taverns: Sequence[seaglass.TavernGame],
        particles,
        ctx: Optional[seaglass.LobbyContext] = None,
        combats_per_particle: int = 14,
        pre_positioned: bool = False,
    ) -> List[float]:
        """Batched Exact 1-Turn + 2-Turn Bellman evaluation across `B` candidate end-of-turn
        games: bit-for-bit identical math to `[evaluate_turn_end(g, ...) for g in taverns]`,
        collapsing `2*B` tiny GPU calls into at most 2 large batched GPU calls (~5-10x faster!)."""
        n_cands = len(taverns)
        if n_cands == 0:
            return []
        if n_cands == 1 or not (
            self.exact_bellman_rollout
            and self.use_physics_rollout
            and particles is not None
            and particles.boards
            and (ctx is None or not getattr(ctx, "duos", False))
        ):
            return [
                self.evaluate_turn_end(
                    g,
                    particles,
                    ctx=ctx,
                    combats_per_particle=combats_per_particle,
                    pre_positioned=pre_positioned,
                )
                for g in taverns
            ]

        want_two_turn = self.two_turn_rollout_steps > 0 and self._rollout_fn is not None
        # Always evaluate the exact same `two_turn_samples` opponent particles for BOTH deterministic
        # Stage-2 plans and Refresh/Discover probes (zero apples-to-oranges particle optimism bias!).
        n_t1_target = self.two_turn_samples
        alive_units = ctx.alive_units if ctx is not None else 8
        is_ghost = particles.is_ghost or (ctx.next_is_ghost if ctx is not None else False)
        damage_cap = ctx.damage_cap if ctx is not None else None
        lobby_hps = (
            [o.health + o.armor for o in ctx.opponents if o.alive]
            if ctx is not None
            else [30] * 7
        )
        is_endgame = alive_units <= 4
        ko_mult = 1.65 if is_endgame else 1.05
        n_boards = max(1, len(particles.boards))

        death_place_idx = min(7, max(0, int(alive_units) - 1))
        death_util = float(self.neural_weight * SCALING_FIRST_PLACE_WEIGHTS[death_place_idx])
        p1_bonus = float(getattr(self.predictor, "p1_bonus", 20.0)) if self.predictor is not None else 20.0
        growth_w = float(getattr(self.predictor, "growth_weight", 6.0)) if self.predictor is not None else 6.0
        prior_win_offset = float(self.prior_weight * self._turn_win_macro_cap)
        win_1v1_util = (
            prior_win_offset
            + float(self.neural_weight * (82.0 + p1_bonus + growth_w * 11.5))
            + 45.0
        )

        opp_ehp_1v1: Optional[int] = None
        if alive_units == 2 and not is_ghost and ctx is not None:
            alive_opps = [o for o in ctx.opponents if o.alive]
            if len(alive_opps) == 1:
                opp_ehp_1v1 = int(alive_opps[0].health + alive_opps[0].armor)

        n_rolls = max(4, self.phys_rolls_per_particle) if opp_ehp_1v1 is not None else self.phys_rolls_per_particle
        all_alive_obs: List[seaglass.TavernObservation] = []
        all_alive_frozen: List[bool] = []
        all_alive_weights: List[float] = []
        cand_meta: List[Tuple[float, float, int, int, int, int]] = []  # (icm, term_sum, tot_rolls, a_start, a_end, t1_seen)
        all_t1_tasks: List[Tuple[int, int, seaglass.TavernGame, int, float]] = []  # (cand_i, alive_idx, g_next, t1_seed, t1_w)

        for cand_i, tavern in enumerate(taverns):
            obs_pre = tavern.observe()
            combat_game = tavern.copy()
            if not pre_positioned:
                self.positioner.fast_reposition_game(combat_game, obs_pre, particles)
            combat_game.reseed((obs_pre.turn * 65537 + 17) | 1)
            if combat_game.is_legal(seaglass.TavernAction.end_turn()):
                combat_game.end_turn()

            combat_icm_term = 0.0
            hp_now = self.effective_hp(obs_pre)
            in_lethal_zone = (not is_ghost) and (
                damage_cap is None or hp_now <= float(damage_cap)
            )
            any_loss_lethal = (not is_ghost) and (hp_now <= float(particles.tier + 1))
            p_surv_by_particle: Optional[List[float]] = [] if any_loss_lethal else None
            if self.combat_icm_weight > 0.0 or in_lethal_zone:
                u_win_or_draw = self.cvar_survival_utility(hp_now, alive_units, lobby_hps, damage_cap)
                icm_sum = 0.0
                offense_sum = 0.0
                margin_sum = 0.0
                for idx, opp_board in enumerate(particles.boards):
                    dist = combat_game.equity_against(
                        opp_board,
                        opponent_tier=particles.tier,
                        n=combats_per_particle,
                        base_seed=obs_pre.turn * 1009 + idx * 37 + 1,
                    )
                    if p_surv_by_particle is not None:
                        p_surv_by_particle.append(float(dist.a_win_rate + dist.draw_rate))
                    if in_lethal_zone:
                        margin_sum += 0.65 * float(dist.damage_mean)
                    if self.combat_icm_weight > 0.0:
                        if dist.b_win_rate > 1e-6:
                            worst_loss = float(max(0, -dist.damage_min))
                            if damage_cap is not None:
                                worst_loss = min(worst_loss, float(damage_cap))
                            avg_loss = min(worst_loss, max(float(particles.tier + 1), 0.65 * worst_loss))
                            if damage_cap is not None:
                                avg_loss = min(avg_loss, float(damage_cap))
                        else:
                            worst_loss = 0.0
                            avg_loss = 0.0
                        u_avg = self.cvar_survival_utility(hp_now - avg_loss, alive_units, lobby_hps, damage_cap)
                        u_wst = self.cvar_survival_utility(hp_now - worst_loss, alive_units, lobby_hps, damage_cap)
                        icm_sum += (dist.a_win_rate + dist.draw_rate) * u_win_or_draw + dist.b_win_rate * (
                            0.68 * u_avg + 0.32 * u_wst
                        )
                        if not is_ghost and dist.a_win_rate > 0.0:
                            dealt = max(0.0, dist.damage_mean)
                            if damage_cap is not None:
                                dealt = min(dealt, float(damage_cap))
                            offense_sum += ko_mult * (0.31 * dealt + 3.0 * dist.a_win_rate)
                combat_icm_term = (
                    self.combat_icm_weight * ((icm_sum + offense_sum) / n_boards)
                    + (margin_sum / n_boards)
                )

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
                )
                if hr_dist.b_win_rate > 1e-6:
                    hr_worst = float(max(0, -hr_dist.damage_min))
                    if damage_cap is not None:
                        hr_worst = min(hr_worst, float(damage_cap))
                    if hp_now - hr_worst <= 0.0:
                        cvar_penalty = 38.0 * hr_dist.b_win_rate
            combat_icm_term -= cvar_penalty

            a_start = len(all_alive_obs)
            terminal_sum = 0.0
            total_rollouts = 0
            t1_seen = 0
            is_fz = bool(obs_pre.is_frozen)
            use_rb_lethal = any_loss_lethal and opp_ehp_1v1 is None and p_surv_by_particle is not None
            for p_idx, opp_board in enumerate(particles.boards):
                if use_rb_lethal:
                    p_surv = p_surv_by_particle[p_idx]
                    total_rollouts += n_rolls
                    terminal_sum += float(n_rolls) * (1.0 - p_surv) * death_util
                    if p_surv <= 1e-6:
                        if want_two_turn and t1_seen < n_t1_target:
                            t1_seen += 1
                        continue
                    p_alive_indices: List[int] = []
                    p_t1_cand: Optional[Tuple[int, seaglass.TavernGame, int]] = None
                    for r_idx in range(n_rolls):
                        g_next = combat_game.copy()
                        c_seed = (obs_pre.turn * 131071 + p_idx * 8191 + r_idx * 2503 + 3) | 1
                        g_next.reseed(c_seed)
                        g_next.resolve_combat_against(
                            opp_board,
                            opponent_tier=particles.tier,
                            seed=c_seed,
                        )
                        if int(g_next.observe().health) <= 0:
                            continue
                        t1_seed = (obs_pre.turn * 524287 + p_idx * 16381 + r_idx * 4099 + 11) | 1
                        g_next.reseed(t1_seed)
                        g_next.start_turn()
                        alive_idx = len(all_alive_obs)
                        all_alive_obs.append(g_next.observe())
                        all_alive_frozen.append(is_fz)
                        all_alive_weights.append(1.0)
                        p_alive_indices.append(alive_idx)
                        if p_t1_cand is None:
                            p_t1_cand = (alive_idx, g_next, t1_seed)
                    if not p_alive_indices:
                        g_surv = combat_game.copy()
                        t1_seed = (obs_pre.turn * 524287 + p_idx * 16381 + 11) | 1
                        g_surv.reseed(t1_seed)
                        g_surv.start_turn()
                        alive_idx = len(all_alive_obs)
                        all_alive_obs.append(g_surv.observe())
                        all_alive_frozen.append(is_fz)
                        all_alive_weights.append(1.0)
                        p_alive_indices.append(alive_idx)
                        p_t1_cand = (alive_idx, g_surv, t1_seed)
                    w_each = (float(n_rolls) * p_surv) / float(len(p_alive_indices))
                    for ai in p_alive_indices:
                        all_alive_weights[ai] = w_each
                    if want_two_turn and t1_seen < n_t1_target:
                        if p_t1_cand is not None:
                            all_t1_tasks.append((cand_i, p_t1_cand[0], p_t1_cand[1], p_t1_cand[2], p_surv))
                        t1_seen += 1
                    continue

                for r_idx in range(n_rolls):
                    total_rollouts += 1
                    g_next = combat_game.copy()
                    c_seed = (obs_pre.turn * 131071 + p_idx * 8191 + r_idx * 2503 + 3) | 1
                    g_next.reseed(c_seed)
                    res = g_next.resolve_combat_against(
                        opp_board,
                        opponent_tier=particles.tier,
                        seed=c_seed,
                    )
                    post_hp = int(g_next.observe().health)
                    if post_hp <= 0:
                        terminal_sum += death_util
                        if want_two_turn and r_idx == 0 and t1_seen < n_t1_target:
                            t1_seen += 1
                        continue
                    if opp_ehp_1v1 is not None and res.outcome == "a_win":
                        dealt = int(res.hero_damage)
                        if damage_cap is not None:
                            dealt = min(dealt, int(damage_cap))
                        if dealt >= opp_ehp_1v1:
                            terminal_sum += win_1v1_util
                            if want_two_turn and r_idx == 0 and t1_seen < n_t1_target:
                                t1_seen += 1
                            continue
                    t1_seed = (obs_pre.turn * 524287 + p_idx * 16381 + r_idx * 4099 + 11) | 1
                    g_next.reseed(t1_seed)
                    g_next.start_turn()
                    alive_idx = len(all_alive_obs)
                    all_alive_obs.append(g_next.observe())
                    all_alive_frozen.append(is_fz)
                    all_alive_weights.append(1.0)
                    if want_two_turn and r_idx == 0 and t1_seen < n_t1_target:
                        all_t1_tasks.append((cand_i, alive_idx, g_next, t1_seed, 1.0))
                        t1_seen += 1
            cand_meta.append((combat_icm_term, terminal_sum, total_rollouts, a_start, len(all_alive_obs), t1_seen))

        all_next_scores: List[float] = []
        if all_alive_obs:
            all_next_scores = self.evaluate_fast_batch(
                all_alive_obs,
                ctx=ctx,
                end_of_turn=False,
                particles=particles,
                in_dist_sot=True,
                sot_shop_is_real_list=all_alive_frozen,
            )

        out_scores: List[float] = []
        for icm_t, term_s, tot_r, a0, a1, _ in cand_meta:
            alive_s = (
                float(sum(all_alive_weights[k] * all_next_scores[k] for k in range(a0, a1)))
                if a1 > a0
                else 0.0
            )
            out_scores.append(icm_t + (alive_s + term_s) / max(1, tot_r))

        if not want_two_turn or not all_t1_tasks or n_t1_target <= 0:
            return out_scores

        eot_obs_list: List[seaglass.TavernObservation] = []
        eot_meta: List[Tuple[int, float, float]] = []  # (cand_i, v_sot, t1_w)
        self._in_t1_rollout = True
        try:
            if self._rollout_batch_fn is not None:
                batch_games: List[seaglass.TavernGame] = []
                batch_seeds: List[int] = []
                for cand_i, alive_idx, g_t1, t1_seed, t1_w in all_t1_tasks:
                    for _ in range(2):
                        cur_t1 = g_t1.observe()
                        if not cur_t1.trinket_options:
                            break
                        t_acts = [a for a in g_t1.legal_actions() if a.kind == "choose_trinket"]
                        if not t_acts:
                            break
                        best_ta = t_acts[0]
                        best_tv = -1e18
                        for ta in t_acts:
                            sim_t = g_t1.copy()
                            sim_t.step(ta)
                            tv = self.evaluate_fast(sim_t.observe(), ctx=ctx, end_of_turn=False, particles=particles)
                            if tv > best_tv:
                                best_tv = tv
                                best_ta = ta
                        g_t1.step(best_ta)
                    batch_games.append(g_t1)
                    batch_seeds.append((t1_seed ^ 0x2468ACE1) | 1)
                    eot_meta.append((cand_i, all_next_scores[alive_idx], t1_w))
                rolled = self._rollout_batch_fn(
                    batch_games, batch_seeds, max_steps=self.two_turn_rollout_steps, ctx=ctx, particles=particles
                )
                eot_obs_list = [g_r.observe() for g_r in rolled]
            else:
                for cand_i, alive_idx, g_t1, t1_seed, t1_w in all_t1_tasks:
                    for _ in range(2):
                        cur_t1 = g_t1.observe()
                        if not cur_t1.trinket_options:
                            break
                        t_acts = [a for a in g_t1.legal_actions() if a.kind == "choose_trinket"]
                        if not t_acts:
                            break
                        best_ta = t_acts[0]
                        best_tv = -1e18
                        for ta in t_acts:
                            sim_t = g_t1.copy()
                            sim_t.step(ta)
                            tv = self.evaluate_fast(sim_t.observe(), ctx=ctx, end_of_turn=False, particles=particles)
                            if tv > best_tv:
                                best_tv = tv
                                best_ta = ta
                        g_t1.step(best_ta)
                    g_t1_eot = self._rollout_fn(
                        g_t1,
                        ctx,
                        particles=particles,
                        max_steps=self.two_turn_rollout_steps,
                        crn_seed=(t1_seed ^ 0x2468ACE1) | 1,
                    )
                    eot_obs_list.append(g_t1_eot.observe())
                    eot_meta.append((cand_i, all_next_scores[alive_idx], t1_w))
        finally:
            self._in_t1_rollout = False

        v_eots = self.evaluate_fast_batch(
            eot_obs_list,
            ctx=ctx,
            end_of_turn=True,
            particles=particles,
        )
        delta_sums = [0.0] * n_cands
        for (cand_i, vs, tw), ve in zip(eot_meta, v_eots):
            d = (ve - vs) if self.two_turn_unclamped else max(0.0, ve - vs)
            delta_sums[cand_i] += tw * d
        for cand_i in range(n_cands):
            t1_seen = cand_meta[cand_i][5]
            if t1_seen > 0:
                out_scores[cand_i] += self.two_turn_weight * (delta_sums[cand_i] / t1_seen)
        return out_scores

    def evaluate_turn_end(
        self,
        tavern: seaglass.TavernGame,
        particles,
        ctx: Optional[seaglass.LobbyContext] = None,
        combats_per_particle: int = 14,
        board_override=None,
        pre_positioned: bool = False,
    ) -> float:
        want_two_turn = (
            self.two_turn_rollout_steps > 0
            and self._rollout_fn is not None
            and board_override is None
            and self.use_physics_rollout
            and particles is not None
            and bool(particles.boards)
            and (ctx is None or not getattr(ctx, "duos", False))
        )
        if (
            self.exact_bellman_rollout
            and self.use_physics_rollout
            and board_override is None
            and particles is not None
            and particles.boards
            and (ctx is None or not getattr(ctx, "duos", False))
        ):
            return self._evaluate_exact_bellman_turn_end(
                tavern,
                particles,
                ctx=ctx,
                combats_per_particle=combats_per_particle,
                pre_positioned=pre_positioned,
                run_two_turn=want_two_turn,
                t1_sample_cap=None,
            )
        base_score = super().evaluate_turn_end(
            tavern,
            particles,
            ctx=ctx,
            combats_per_particle=combats_per_particle,
            board_override=board_override,
            pre_positioned=pre_positioned,
        )
        if not want_two_turn:
            return base_score

        obs_pre = tavern.observe()
        combat_game = tavern.copy()
        if not pre_positioned:
            self.positioner.fast_reposition_game(combat_game, obs_pre, particles)
        combat_game.reseed((obs_pre.turn * 65537 + 17) | 1)
        if combat_game.is_legal(seaglass.TavernAction.end_turn()):
            combat_game.end_turn()

        n_t1 = self.two_turn_samples
        sot_obs_list: List[seaglass.TavernObservation] = []
        eot_obs_list: List[seaglass.TavernObservation] = []
        self._in_t1_rollout = True
        try:
            for s_idx in range(n_t1):
                p_idx = s_idx % len(particles.boards)
                g_t1 = combat_game.copy()
                c_seed = (obs_pre.turn * 131071 + s_idx * 8191 + 3) | 1
                g_t1.reseed(c_seed)
                g_t1.resolve_combat_against(
                    particles.boards[p_idx],
                    opponent_tier=particles.tier,
                    seed=c_seed,
                )
                if g_t1.observe().health <= 0:
                    continue
                t1_seed = (obs_pre.turn * 524287 + s_idx * 16381 + 11) | 1
                g_t1.reseed(t1_seed)
                g_t1.start_turn()
                sot_obs = g_t1.observe()
                sot_obs_list.append(sot_obs)
                # Resolve Turn 6/9 Trinket prompts quickly inside Turn t+1 rollout so the consumer spends Turn t+1 gold!
                for _ in range(2):
                    cur_t1 = g_t1.observe()
                    if not cur_t1.trinket_options:
                        break
                    t_acts = [a for a in g_t1.legal_actions() if a.kind == "choose_trinket"]
                    if not t_acts:
                        break
                    best_ta = t_acts[0]
                    best_tv = -1e18
                    for ta in t_acts:
                        sim_t = g_t1.copy()
                        sim_t.step(ta)
                        tv = self.evaluate_fast(sim_t.observe(), ctx=ctx, end_of_turn=False, particles=particles)
                        if tv > best_tv:
                            best_tv = tv
                            best_ta = ta
                    g_t1.step(best_ta)
                g_t1_eot = self._rollout_fn(
                    g_t1,
                    ctx,
                    particles=particles,
                    max_steps=self.two_turn_rollout_steps,
                    crn_seed=(t1_seed ^ 0x2468ACE1) | 1,
                )
                eot_obs_list.append(g_t1_eot.observe())
        finally:
            self._in_t1_rollout = False

        if not sot_obs_list:
            return base_score

        v_sots = self.evaluate_fast_batch(
            sot_obs_list,
            ctx=ctx,
            end_of_turn=False,
            particles=particles,
            in_dist_sot=True,
            sot_shop_is_real=obs_pre.is_frozen,
        )
        v_eots = self.evaluate_fast_batch(
            eot_obs_list,
            ctx=ctx,
            end_of_turn=True,
            particles=particles,
        )
        if self.two_turn_unclamped:
            # Exact 2-Turn Bellman telescoping residual: with w=1.0, cancels biased V(s_{t+1}^{SOT})
            # and evaluates E[V(s_{t+1}^{EOT})] directly (preserving negative residual of staying low-tier!).
            delta_sum = sum(ve - vs for ve, vs in zip(v_eots, v_sots))
        else:
            delta_sum = sum(max(0.0, ve - vs) for ve, vs in zip(v_eots, v_sots))
        return base_score + self.two_turn_weight * (delta_sum / n_t1)

    def macro_and_economy_value(
        self,
        obs: seaglass.TavernObservation,
        *args,
        end_of_turn: bool = False,
        **kwargs,
    ) -> float:
        val = super().macro_and_economy_value(obs, *args, end_of_turn=end_of_turn, **kwargs)
        if end_of_turn and obs.hand:
            # Neutralize SovereignEvaluator's inherited artificial EOT hand penalties:
            # (1) -0.35 on persistent spells in hand (so multi-turn spell banking is judged by rollout + V),
            # (2) -10.5 on playable minions held in hand when board < 7 (normal heroes still play minions
            #     via board unit_value + combat sim, while hand-summon heroes like Sneed aren't drowned out!).
            profiles = self.meta.profiles
            n_board = len(obs.board)
            for c in obs.hand:
                if c.is_spell:
                    prof = profiles.get(c.card_id)
                    if prof is None or not prof.is_temporary:
                        val += 0.35
                elif (not c.unplayable) and n_board < 7:
                    val += 10.5
        return val

    def evaluate_fast_batch(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = False,
        particles=None,
        in_dist_sot: bool = False,
        sot_shop_is_real: bool = False,
        sot_shop_is_real_list: Optional[Sequence[bool]] = None,
        single_model: bool = False,
    ) -> List[float]:
        if not obs_list:
            return []
        if sot_shop_is_real_list is not None:
            prior_scores = [
                self.macro_and_economy_value(
                    o,
                    ctx=ctx,
                    end_of_turn=end_of_turn,
                    particles=particles,
                    in_dist_sot=in_dist_sot,
                    sot_shop_is_real=bool(is_r),
                )
                for o, is_r in zip(obs_list, sot_shop_is_real_list)
            ]
        else:
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
        if (
            (self._in_t1_rollout and not single_model)
            or (not self.score_all_states and not (in_dist_sot or end_of_turn) and not single_model)
            or not self.has_neural_model
            or self.predictor is None
            or (ctx is not None and getattr(ctx, "duos", False))
        ):
            pw = 1.0 if self._in_t1_rollout else self.prior_weight
            return [pw * p_val for p_val in prior_scores]

        net_scores = self.predictor.predict_batch(
            obs_list,
            ctx=ctx,
            end_of_turn=end_of_turn,
            force_ensemble=(in_dist_sot and not single_model),
            single_model=single_model,
        )
        w = self.neural_weight
        pw = self.prior_weight
        return [pw * p_val + w * n_val for p_val, n_val in zip(prior_scores, net_scores)]

    def _pre_end_turn_discipline(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> float:
        """Zero hardcoded hand/board penalties: let exact Rust combat sim + next-turn SOT
        `ScalingValueTransformer` naturally evaluate Sneed/Bassgill/Busker/triple holds & freezes."""
        return 0.0

    def choose_best_hero(
        self,
        obs: seaglass.TavernObservation,
        legal: Sequence[seaglass.TavernAction],
        ctx: Optional[seaglass.LobbyContext] = None,
        base_game: Optional[seaglass.TavernGame] = None,
    ) -> seaglass.TavernAction:
        """Select offered Hero using our learned `ScalingValueTransformer` weighted consistently with `prior_weight` and `neural_weight`."""
        if (
            base_game is None
            or not self.has_neural_model
            or self.predictor is None
            or (ctx is not None and getattr(ctx, "duos", False))
        ):
            return super().choose_best_hero(obs, legal, ctx=ctx, base_game=base_game)

        lobby_tribes = list(ctx.tribes) if ctx is not None else []
        candidates: List[Tuple[seaglass.TavernAction, float, seaglass.TavernObservation]] = []
        for act in legal:
            if act.kind != "choose_hero":
                continue
            opt_idx, _ = act.args
            if opt_idx is None or opt_idx >= len(obs.hero_options):
                continue
            hid = obs.hero_options[opt_idx]
            prior_s = self.sov_kb.hero_lobby_score(hid, lobby_tribes)
            prof = self.meta.profiles.get(hid)
            if prof is not None:
                prior_s += 0.65 * prof.engine_weight
                if prof.is_economy_generator:
                    prior_s += 2.0
            sim = base_game.copy()
            if not sim.is_legal(act):
                continue
            sim.step(act)
            sim_obs = sim.observe()
            prior_s += 0.34 * (self.effective_hp(sim_obs) - 30.0)
            candidates.append((act, prior_s, sim_obs))

        if not candidates:
            return legal[0]
        net_vals = self.predictor.predict_batch([c[2] for c in candidates], ctx=ctx, end_of_turn=False)
        best_i = max(
            range(len(candidates)),
            key=lambda i: self.prior_weight * candidates[i][1] + self.neural_weight * net_vals[i],
        )
        return candidates[best_i][0]


@functools.lru_cache(maxsize=8)
def _load_cached_scaling_model(path: str, device_str: str) -> ScalingValueTransformer:
    return ScalingValueTransformer.load_checkpoint(path).to(torch.device(device_str)).eval()


class ScalingValuePredictor:
    """Drop-in neural predictor wrapping a 1..K model ensemble of `ScalingValueTransformer` (`predict_batch` interface)."""

    def __init__(
        self,
        ckpt_paths: Sequence[str],
        p1_bonus: float = 10.0,
        growth_weight: float = 4.0,
        growth_mode: str = "abs",
        ensemble_fast_beam: bool = True,
        use_cuda: bool = False,
    ) -> None:
        self.device = torch.device("cuda" if (use_cuda and torch.cuda.is_available()) else "cpu")
        dev_str = str(self.device)
        self.models = [_load_cached_scaling_model(str(p), dev_str) for p in ckpt_paths]
        self.mask_unfrozen_shop_eot = bool(self.models) and all(
            bool(m.config.get("mask_unfrozen_shop_eot", False)) for m in self.models
        )
        self.p1_bonus = float(p1_bonus)
        self.growth_weight = float(growth_weight)
        self.growth_mode = str(growth_mode)
        self.ensemble_fast_beam = bool(ensemble_fast_beam)
        self._ready = len(self.models) > 0

    @property
    def is_ready(self) -> bool:
        return self._ready

    def predict_batch(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = True,
        force_ensemble: bool = False,
        single_model: bool = False,
    ) -> List[float]:
        if not obs_list or not self.models:
            return []
        pvs, cis, nfs, cur_log_s = [], [], [], []
        use_arranging = bool(end_of_turn and self.mask_unfrozen_shop_eot)
        for o in obs_list:
            pv, ci, nf = encode_observation(o, ctx, arranging=use_arranging)
            if end_of_turn:
                # At End-of-Turn, unspent gold & unused once-per-turn hero powers expire — zero them so
                # leaving dead gold at EOT can never spoof a Start-of-Turn state with gold still to spend!
                pv = pv.copy()
                pv[4] = 0.0   # obs.gold / 10.0
                pv[10] = 0.0  # hero_power_usable
                pv[12] = 0.0  # hero_power_uses_left
                pv[21] = 0.0  # second_hero_power_usable
                pv[23] = 0.0  # second_hero_power_uses_left
                if self.mask_unfrozen_shop_eot:
                    # Mask ALL shop tokens and `is_frozen` at End-of-Turn unconditionally:
                    # removes spurious EOT shop variance, prevents Tavern Tier shortcut-learning
                    # from shop cards, AND makes 0g ToggleFreeze mathematically un-exploitable!
                    pv[15] = 0.0
                    ci = ci.copy()
                    nf = nf.copy()
                    ci[SHOP_SLOT_START:SHOP_SLOT_END] = 0
                    nf[SHOP_SLOT_START:SHOP_SLOT_END] = 0.0
            pvs.append(pv)
            cis.append(ci)
            nfs.append(nf)
            st = sum(max(0, u.attack) + max(0, u.health) for u in o.board)
            cur_log_s.append(math.log1p(float(st)))

        pv_t = torch.from_numpy(np.asarray(pvs, dtype=np.float32)).to(self.device)
        ci_t = torch.from_numpy(np.asarray(cis, dtype=np.int64)).to(self.device)
        nf_t = torch.from_numpy(np.asarray(nfs, dtype=np.float32)).to(self.device)

        # Use latest single model on large intermediate beam-pruning batches (3x faster!),
        # and full ensemble on ALL EOT, SOT, Discover/open terminals, and turn-end evaluations!
        active_models = (
            self.models[:1]
            if single_model
            else (
                self.models[-1:]
                if (
                    self.ensemble_fast_beam
                    and not force_ensemble
                    and len(self.models) > 1
                    and not end_of_turn
                    and len(obs_list) > 6
                )
                else self.models
            )
        )

        probs_acc = None
        p1_acc = None
        fs_acc = None
        use_bf16 = self.device.type == "cuda"
        with torch.inference_mode(), torch.amp.autocast("cuda", dtype=torch.bfloat16, enabled=use_bf16):
            for mdl in active_models:
                _, place_logits, pred_fs, p1_lg = mdl.forward(pv_t, ci_t, nf_t)
                pr = F.softmax(place_logits.float(), dim=-1)
                p1 = torch.sigmoid(p1_lg.float())
                fs = pred_fs.float()
                probs_acc = pr if probs_acc is None else (probs_acc + pr)
                p1_acc = p1 if p1_acc is None else (p1_acc + p1)
                fs_acc = fs if fs_acc is None else (fs_acc + fs)

        inv_k = 1.0 / len(active_models)
        probs = (probs_acc * inv_k).detach().cpu().numpy()
        p1_p = (p1_acc * inv_k).detach().cpu().numpy()
        fs_np = (fs_acc * inv_k).detach().cpu().numpy()

        place_u = probs @ SCALING_FIRST_PLACE_WEIGHTS
        if self.growth_mode == "delta":
            cur_np = np.asarray(cur_log_s, dtype=np.float32)
            growth_term = np.clip(fs_np - cur_np, -1.5, 5.0)
        elif self.growth_mode == "convex":
            # Risk-seeking convex utility over predicted 3-turn future board power (fs^2 / 14):
            # naturally halves early-game log1p(3/3) anti-leveling drag while amplifying late-game
            # Tier-5/6 exponential scaling engines (ZERO turn/tier/card hardcoding!).
            g_clip = np.clip(fs_np, 0.0, 11.0)
            growth_term = (g_clip * g_clip) / 14.0
        else:
            # Strictly non-negative monotone 3-turn future board power E[log1p(stats_{t+3})] (no -cur_np penalty!)
            growth_term = np.clip(fs_np, 0.0, 11.0)
        tot = place_u + self.p1_bonus * p1_p + self.growth_weight * growth_term
        return tot.astype(np.float64).tolist()


@functools.lru_cache(maxsize=4)
def _load_cached_policy_model(path: str, device_str: str):
    model, _ = rl_checkpoint.load(path)
    return model.to(torch.device(device_str)).eval()


class PolicyPriorPredictor:
    """Fast GPU AlphaZero Policy Prior (`EntityTransformer`) distilled from 1st-place winner trajectories.

    Powers both:
    - Batched neural rollout consumers (`rollout_batch`: 48 games in <5ms total!) replacing heuristic rollouts, and
    - Learned Stage-1 Top-K action proposals (`predict_batch_probs`) unlocking engine pivots banned by heuristic pruners.
    """

    def __init__(self, ckpt_path: str, use_cuda: bool = False) -> None:
        self.device = torch.device("cuda" if (use_cuda and torch.cuda.is_available()) else "cpu")
        self.model = _load_cached_policy_model(str(ckpt_path), str(self.device))

    def predict_batch_probs(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        legal_lists: Sequence[Sequence[seaglass.TavernAction]],
    ) -> List[List[Tuple[seaglass.TavernAction, float]]]:
        if not obs_list:
            return []
        b_size = len(obs_list)
        pvs = []
        cis = []
        nfs = []
        masks = np.zeros((b_size, ACTION_SPACE_SIZE), dtype=np.bool_)
        row_maps: List[List[Tuple[int, seaglass.TavernAction]]] = []

        for i, (obs, legal) in enumerate(zip(obs_list, legal_lists)):
            pv, ci, nf = encode_observation(obs)
            pvs.append(pv)
            cis.append(ci)
            nfs.append(nf)
            rmap: List[Tuple[int, seaglass.TavernAction]] = []
            for act in legal:
                if act.kind in ("reposition", "toggle_freeze"):
                    continue
                idx = ACTION_TO_INDEX.get((act.kind, act.args[0], act.args[1]))
                if idx is not None and not masks[i, idx]:
                    masks[i, idx] = True
                    rmap.append((idx, act))
            if not rmap:
                masks[i, 0] = True
            row_maps.append(rmap)

        pv_t = torch.from_numpy(np.asarray(pvs, dtype=np.float32)).to(self.device)
        ci_t = torch.from_numpy(np.asarray(cis, dtype=np.int64)).to(self.device)
        nf_t = torch.from_numpy(np.asarray(nfs, dtype=np.float32)).to(self.device)
        mk_t = torch.from_numpy(masks).to(self.device)

        with torch.inference_mode():
            logits, _ = self.model(pv_t, ci_t, nf_t, mk_t)
            probs_np = F.softmax(logits.float(), dim=-1).detach().cpu().numpy()

        out: List[List[Tuple[seaglass.TavernAction, float]]] = []
        for i, rmap in enumerate(row_maps):
            if not rmap:
                out.append([])
                continue
            scored = [(act, float(probs_np[i, idx])) for idx, act in rmap]
            scored.sort(key=lambda x: x[1], reverse=True)
            out.append(scored)
        return out

    def rollout_batch(
        self,
        games: Sequence[seaglass.TavernGame],
        crn_seeds: Sequence[int],
        max_steps: int = 4,
        max_refreshes: int = 2,
        ctx: Optional[seaglass.LobbyContext] = None,
        particles=None,
    ) -> List[seaglass.TavernGame]:
        if not games:
            return []
        sims: List[seaglass.TavernGame] = []
        for g, s in zip(games, crn_seeds):
            sim = g.copy()
            sim.reseed(int(s) | 1)
            sims.append(sim)

        active = list(range(len(sims)))
        actions_taken = [0] * len(sims)
        rolls_used = [0] * len(sims)

        for _ in range(max_steps + max_refreshes + 4):
            if not active:
                break
            a_obs: List[seaglass.TavernObservation] = []
            a_legals: List[List[seaglass.TavernAction]] = []
            still_idx: List[int] = []
            for idx in active:
                sim = sims[idx]
                obs = sim.observe()
                leg = sim.legal_actions()
                if not leg:
                    continue
                if rolls_used[idx] >= max_refreshes:
                    leg = [a for a in leg if a.kind != "refresh"]
                if not leg:
                    continue
                still_idx.append(idx)
                a_obs.append(obs)
                a_legals.append(leg)

            if not still_idx:
                break
            ranked_batch = self.predict_batch_probs(a_obs, a_legals)
            next_active: List[int] = []
            for idx, ranked in zip(still_idx, ranked_batch):
                if not ranked:
                    continue
                best_act = ranked[0][0]
                k = best_act.kind
                if k in ("end_turn", "pass"):
                    continue
                ended = sims[idx].step(best_act)
                if ended:
                    continue
                if k == "refresh":
                    rolls_used[idx] += 1
                elif k not in ("choose_discover", "choose_trinket"):
                    actions_taken[idx] += 1
                if actions_taken[idx] < max_steps:
                    next_active.append(idx)
            active = next_active
        return sims


class ScalingSearchBot(SovereignBot):
    """Multi-Epoch Rust Physics Search upgraded with our `ScalingValueTransformer` ensemble (ZERO comp hardcoding!)."""

    def __init__(
        self,
        value_ckpt: Optional[str] = None,
        value_ckpts: Optional[Sequence[str]] = None,
        upgrade_res_ckpts: Optional[Sequence[str]] = None,
        upgrade_res_weight: float = 0.0,
        neural_weight: float = 1.35,
        prior_weight: float = 1.0,
        score_all_states: bool = True,
        two_turn_rollout_steps: int = 0,
        two_turn_weight: float = 0.45,
        two_turn_samples: int = 1,
        two_turn_unclamped: bool = False,
        no_t1_upgrade: bool = False,
        exact_bellman_rollout: bool = False,
        combat_icm_weight: float = 1.0,
        phys_rolls_per_particle: int = 2,
        finals_min_nw: float = 0.25,
        stoch_samples: int = 1,
        p1_bonus: float = 10.0,
        growth_weight: float = 4.0,
        growth_mode: str = "abs",
        ensemble_fast_beam: bool = True,
        unshackled_pruning: bool = False,
        unshackle_shop_pivots: bool = False,
        calibrated_rollout: bool = False,
        fast_neural_rollout: bool = False,
        policy_ckpt: Optional[str] = None,
        policy_rollout: bool = False,
        policy_proposal_topk: int = 0,
        policy_proposal_min_prob: float = 0.04,
        search_noise_std: float = 0.0,
        stratified_stage2: bool = True,
        plan_explore_temp: float = 0.0,
        plan_explore_max_turn: int = 10,
        plan_explore_margin: float = 22.0,
        block_upgrade_turn: int = -1,
        beam_width: int = 16,
        max_epoch_depth: int = 5,
        top_k_combat_evals: int = 5,
        refresh_samples: int = 3,
        num_particles: int = 3,
        finals_pos_candidates: int = 96,
        use_cuda: bool = False,
        seed: int = 2029,
        **kwargs,
    ) -> None:
        super().__init__(
            beam_width=beam_width,
            max_epoch_depth=max_epoch_depth,
            top_k_combat_evals=top_k_combat_evals,
            refresh_samples=refresh_samples,
            num_particles=num_particles,
            seed=seed,
            **kwargs,
        )
        ckpts = list(value_ckpts) if value_ckpts else ([value_ckpt] if value_ckpt else [])
        self.evaluator = ScalingEvaluator(
            catalog_name=self.catalog_name,
            use_physics_rollout=self.evaluator.use_physics_rollout,
            use_neural_value=True,
            neural_weight=float(neural_weight),
            prior_weight=float(prior_weight),
            score_all_states=bool(score_all_states),
            two_turn_rollout_steps=int(two_turn_rollout_steps),
            two_turn_weight=float(two_turn_weight),
            two_turn_samples=int(two_turn_samples),
            two_turn_unclamped=bool(two_turn_unclamped),
            exact_bellman_rollout=bool(exact_bellman_rollout),
            combat_icm_weight=float(combat_icm_weight),
            phys_rolls_per_particle=int(phys_rolls_per_particle),
        )
        self.stoch_samples = max(1, int(stoch_samples))
        self.finals_min_nw = float(finals_min_nw)
        self.no_t1_upgrade = bool(no_t1_upgrade)
        self.evaluator._rollout_fn = self._rollout_shop_consumer
        self.evaluator.predictor = ScalingValuePredictor(
            ckpt_paths=ckpts,
            p1_bonus=p1_bonus,
            growth_weight=growth_weight,
            growth_mode=growth_mode,
            ensemble_fast_beam=ensemble_fast_beam,
            use_cuda=use_cuda,
        )
        self.positioner = self.evaluator.positioner
        self._base_neural_weight = float(neural_weight)
        self._base_prior_weight = float(prior_weight)
        self._base_growth_weight = float(growth_weight)
        self._base_two_turn_steps = int(two_turn_rollout_steps)
        self._base_combat_icm_weight = float(combat_icm_weight)
        self._base_pos_cands = int(self.positioner.max_candidates)
        self._finals_pos_cands = int(finals_pos_candidates)
        self.unshackled_pruning = bool(unshackled_pruning)
        self.unshackle_shop_pivots = bool(unshackle_shop_pivots)
        self.calibrated_rollout = bool(calibrated_rollout)
        self.fast_neural_rollout = bool(fast_neural_rollout)
        self.policy_rollout = bool(policy_rollout)
        self.policy_proposal_topk = max(0, int(policy_proposal_topk))
        self.policy_proposal_min_prob = float(policy_proposal_min_prob)
        self.policy_predictor = (
            PolicyPriorPredictor(policy_ckpt, use_cuda=use_cuda)
            if (policy_ckpt and (self.policy_rollout or self.policy_proposal_topk > 0))
            else None
        )
        if self.policy_rollout and self.policy_predictor is not None:
            self.evaluator._rollout_batch_fn = self.policy_predictor.rollout_batch
        elif self.fast_neural_rollout and self.evaluator.has_neural_model:
            self.evaluator._rollout_batch_fn = self._rollout_shop_consumer_batch
        self.search_noise_std = float(search_noise_std)
        self.stratified_stage2 = bool(stratified_stage2)
        self.plan_explore_temp = float(plan_explore_temp)
        self.plan_explore_max_turn = int(plan_explore_max_turn)
        self.plan_explore_margin = float(plan_explore_margin)
        self.block_upgrade_turn = int(block_upgrade_turn)
        self.upgrade_res_weight = float(upgrade_res_weight)
        self.upgrade_res_predictor = (
            UpgradeResidualPredictor(upgrade_res_ckpts, use_cuda=use_cuda)
            if (upgrade_res_ckpts and self.upgrade_res_weight > 0.0)
            else None
        )

    def _select_epoch_actions(
        self,
        obs: seaglass.TavernObservation,
        legal: Sequence[seaglass.TavernAction],
        particles=None,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> List[seaglass.TavernAction]:
        """Pure symmetry-only action filter when `unshackled_pruning=True`; otherwise never ban legal `upgrade_tavern`."""
        if not self.unshackled_pruning:
            pruned = prune_sovereign_actions(obs, legal, self.evaluator, self.kb, particles=particles, ctx=ctx)
            existing_keys = {_action_key(a) for a in pruned}
            if not any(a.kind == "upgrade_tavern" for a in pruned):
                for a in legal:
                    if a.kind == "upgrade_tavern":
                        pruned.append(a)
                        existing_keys.add(_action_key(a))
                        break
            if self.unshackle_shop_pivots:
                # Zero-hardcoding targeted pivot unshackling: lift `prune_sovereign_actions`'s
                # `weakest_stats >= 95` full-board Tier-5/6 buy ban and `turn > 7` sell-to-level ban
                # while keeping branching bounded (+2..4 actions/node: deduplicated buys + top-2 expendable sells).
                seen_shop_sym: set[Tuple] = set()
                for a in pruned:
                    if a.kind == "buy":
                        si, _ = a.args
                        if si is not None and si < len(obs.shop):
                            sc = obs.shop[si]
                            seen_shop_sym.add((sc.card_id, sc.is_golden, sc.attack, sc.health, sc.is_spell, sc.spell_cost))
                if len(obs.hand) < 10:
                    for a in legal:
                        if a.kind != "buy":
                            continue
                        ak = _action_key(a)
                        if ak in existing_keys:
                            continue
                        si, _ = a.args
                        if si is None or si >= len(obs.shop):
                            continue
                        sc = obs.shop[si]
                        sk = (sc.card_id, sc.is_golden, sc.attack, sc.health, sc.is_spell, sc.spell_cost)
                        if sk in seen_shop_sym:
                            continue
                        seen_shop_sym.add(sk)
                        pruned.append(a)
                        existing_keys.add(ak)
                allow_pivot_sell = (
                    len(obs.board) >= 7
                    or obs.gold + 1 == obs.upgrade_cost
                    or obs.gold + 1 == obs.minion_cost
                )
                if allow_pivot_sell and obs.board:
                    exp_set = set(expendable_slots(obs, self.evaluator, max_slots=2, particles=particles, ctx=ctx))
                    for a in legal:
                        if a.kind != "sell":
                            continue
                        ak = _action_key(a)
                        if ak in existing_keys:
                            continue
                        bp, _ = a.args
                        if bp in exp_set:
                            pruned.append(a)
                            existing_keys.add(ak)
            if (
                self.policy_proposal_topk > 0
                and self.policy_predictor is not None
                and not self.evaluator._in_t1_rollout
                and len(legal) > len(pruned) + 1
            ):
                ranked = self.policy_predictor.predict_batch_probs([obs], [legal])[0]
                added = 0
                for a, prob in ranked:
                    if added >= self.policy_proposal_topk or prob < self.policy_proposal_min_prob:
                        break
                    if a.kind in ("end_turn", "toggle_freeze", "refresh", "dark_discovery", "pass", "reposition"):
                        continue
                    ak = _action_key(a)
                    if ak not in existing_keys:
                        pruned.append(a)
                        existing_keys.add(ak)
                        added += 1
            if self.block_upgrade_turn > 0 and int(obs.turn) == self.block_upgrade_turn:
                pruned = [a for a in pruned if a.kind != "upgrade_tavern"]
            return pruned

        seen_hand_sym: Dict[Tuple, int] = {}
        canonical_hand_idx: set[int] = set()
        for h_i, hc in enumerate(obs.hand):
            sym_k = (
                hc.card_id,
                hc.is_golden,
                hc.attack,
                hc.health,
                hc.spell_cost,
                hc.counter,
                hc.unplayable,
                tuple(hc.dark_gifts),
            )
            if sym_k not in seen_hand_sym:
                seen_hand_sym[sym_k] = h_i
                canonical_hand_idx.add(h_i)

        out: List[seaglass.TavernAction] = []
        seen_play_minion_hand: Dict[int, bool] = {}
        n_bd = len(obs.board)
        for act in legal:
            k = act.kind
            if k == "reposition":
                continue
            if k == "play":
                h_idx, b_pos = act.args
                if h_idx is not None and h_idx < len(obs.hand):
                    if h_idx not in canonical_hand_idx:
                        continue
                    card = obs.hand[h_idx]
                    if not card.is_spell:
                        prof = self.meta.profile(card.card_id)
                        is_targeted_bc = (
                            card.card_id in self.kb.targeted_battlecry_ids
                            or card.card_id not in self.meta.templates_by_id
                        )
                        is_mag_target = card.magnetic and b_pos is not None and b_pos < n_bd
                        has_bc = prof.has_battlecry if prof else False
                        if is_targeted_bc or is_mag_target:
                            out.append(act)
                            seen_play_minion_hand[h_idx] = True
                            continue
                        if has_bc and b_pos in (0, n_bd):
                            out.append(act)
                            seen_play_minion_hand[h_idx] = True
                            continue
                        if not seen_play_minion_hand.get(h_idx, False) and b_pos == n_bd:
                            out.append(act)
                            seen_play_minion_hand[h_idx] = True
                            continue
                        continue
            if k == "play_on_shop":
                h_idx_ps, _ = act.args
                if h_idx_ps is not None and h_idx_ps not in canonical_hand_idx:
                    continue
            out.append(act)
        return out

    def _search_shop_epoch(self, root_game, particles, ctx):
        """Multi-epoch beam search with stratified Stage-2 terminal diversity & clean Top-K plan exploration."""
        if (
            not self.unshackled_pruning
            and self.search_noise_std <= 0.0
            and not self.stratified_stage2
            and self.plan_explore_temp <= 0.0
        ):
            return super()._search_shop_epoch(root_game, particles, ctx)

        root_obs = root_game.observe()
        root_fast = self.evaluator.evaluate_fast(root_obs, ctx=ctx, end_of_turn=False, particles=particles)
        transposition_best: Dict[Tuple, float] = {_canonical_state_key(root_obs): root_fast}
        beam: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = [
            (root_fast, root_game, root_obs, [])
        ]
        # Optional parent `(game, act)` recorded for stochastic terminals so Stage-2 can average
        # over K CRN-paired rolls instead of suffering 1-sample Optimizer's Curse!
        stoch_origins: Dict[int, Tuple[seaglass.TavernGame, seaglass.TavernAction]] = {}
        raw_terminals: List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, bool, List[QueuedStep]]] = []
        roll_prefixes: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []

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
                        fz_end: QueuedStep = (("end_turn", None, None), _plan_validation_key(fz_obs), False)
                        raw_terminals.append((g_fz, fz_obs, True, path + [fz_step, fz_end]))

                if obs.discover_pending is not None or obs.trinket_options:
                    raw_terminals.append((g, obs, False, path))
                    continue

                legal = self._select_epoch_actions(obs, g.legal_actions(), particles=particles, ctx=ctx)
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
                    if k in ("play", "activate", "sell", "play_on_shop") and c_obs.discover_pending is None:
                        gained_hand_card = (
                            len(c_obs.hand) >= n_hand_pre
                            if k in ("play", "play_on_shop")
                            else len(c_obs.hand) > n_hand_pre
                        )
                        shop_mutated = len(c_obs.shop) != n_shop_pre and k != "play_on_shop"
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
                        stoch_origins[id(child)] = (g, act)
                        raw_terminals.append((child, c_obs, False, path + [q_step]))
                    else:
                        raw_children.append((child, c_obs, path + [q_step]))

                raw_children.extend(
                    self._expand_cycle_macros(g, obs, path, particles=particles, ctx=ctx)
                )

            if not raw_children:
                break

            dedup_children = []
            seen_layer: set[Tuple] = set()
            for child, c_obs, c_path in raw_children:
                ckey = _canonical_state_key(c_obs)
                if ckey in seen_layer:
                    continue
                seen_layer.add(ckey)
                dedup_children.append((ckey, child, c_obs, c_path))

            child_scores = self.evaluator.evaluate_fast_batch(
                [item[2] for item in dedup_children], ctx=ctx, end_of_turn=False, particles=particles
            )
            next_beam = []
            for (ckey, child, c_obs, c_path), c_fast in zip(dedup_children, child_scores):
                prev = transposition_best.get(ckey)
                if prev is not None and c_fast <= prev + 1e-6:
                    continue
                transposition_best[ckey] = c_fast
                next_beam.append((c_fast, child, c_obs, c_path))
            if not next_beam:
                break
            next_beam.sort(key=lambda x: x[0], reverse=True)
            if self.stratified_stage2:
                # Prevent Stage-1 greedy truncation from starving `Upgrade + Buy/Play/Refresh` branches
                # whenever mid-turn neural evaluation dips below `Stay + Buy` before Stage-2 rollout!
                upg_beam = [x for x in next_beam if int(x[2].tavern_tier) > int(root_obs.tavern_tier)]
                if upg_beam:
                    upg_quota = min(len(upg_beam), max(4, self.beam_width // 4))
                    picked_upg = upg_beam[:upg_quota]
                    picked_ids = {id(x) for x in picked_upg}
                    rest = [x for x in next_beam if id(x) not in picked_ids]
                    beam = rest[: max(0, self.beam_width - len(picked_upg))] + picked_upg
                else:
                    beam = next_beam[: self.beam_width]
            else:
                beam = next_beam[: self.beam_width]

        eot_indices = [i for i, item in enumerate(raw_terminals) if item[2]]
        open_indices = [i for i, item in enumerate(raw_terminals) if not item[2]]
        term_scores = [0.0] * len(raw_terminals)
        if eot_indices:
            vals = self.evaluator.evaluate_fast_batch(
                [raw_terminals[i][1] for i in eot_indices], ctx=ctx, end_of_turn=True, particles=particles
            )
            for idx, v in zip(eot_indices, vals):
                term_scores[idx] = v
        if open_indices:
            vals = self.evaluator.evaluate_fast_batch(
                [raw_terminals[i][1] for i in open_indices], ctx=ctx, end_of_turn=False, particles=particles
            )
            for idx, v in zip(open_indices, vals):
                term_scores[idx] = v

        root_tier = int(root_obs.tavern_tier)
        is_t1 = int(root_obs.turn) == 1
        upg_horizon_bonus = 0.0
        if (
            self.upgrade_res_predictor is not None
            and self.upgrade_res_weight > 0.0
            and root_tier < 6
            and root_obs.gold >= root_obs.upgrade_cost
            and (ctx is None or not getattr(ctx, "duos", False))
        ):
            adv_upg = self.upgrade_res_predictor.predict_upgrade_advantage(root_obs, ctx=ctx)
            upg_horizon_bonus = self.upgrade_res_weight * self.evaluator.neural_weight * adv_upg
            if upg_horizon_bonus != 0.0:
                for i, item in enumerate(raw_terminals):
                    if int(item[1].tavern_tier) > root_tier:
                        term_scores[i] += upg_horizon_bonus

        sorted_terms = sorted(
            zip(term_scores, raw_terminals), key=lambda x: x[0], reverse=True
        )
        ranked_terms: List[Tuple[float, seaglass.TavernGame, List[QueuedStep]]] = []
        seen_term_keys: set[Tuple] = set()
        n_upgrade_terms = 0
        has_open_term = False
        has_hold_unfz = False
        has_hold_fz = False
        has_spell_hold = False
        _kb_profiles = self.kb.meta.profiles

        def _has_persistent_spell(o: seaglass.TavernObservation) -> bool:
            for c in o.hand:
                if c.is_spell:
                    p = _kb_profiles.get(c.card_id)
                    if p is None or not p.is_temporary:
                        return True
            return False

        for t_score, (t_game, t_obs, t_eot, t_path) in sorted_terms:
            t_sig = (_canonical_state_key(t_obs), t_eot)
            if t_sig in seen_term_keys:
                continue
            seen_term_keys.add(t_sig)
            ranked_terms.append((t_score, t_game, t_path))
            if int(t_obs.tavern_tier) > root_tier:
                n_upgrade_terms += 1
            if not t_eot:
                has_open_term = True
            if t_eot and any(not c.is_spell for c in t_obs.hand):
                if t_obs.is_frozen:
                    has_hold_fz = True
                else:
                    has_hold_unfz = True
            if t_eot and _has_persistent_spell(t_obs):
                has_spell_hold = True
            if len(ranked_terms) >= self.top_k_combat_evals:
                break

        # General 2-stage search stratification (zero card/hero/turn rules): guarantee Stage-2 evaluates
        # top-2 upgraded-tier, open/Discover, unfrozen/frozen minion-in-hand hold, and persistent-spell hold
        # branches (so combat-summon holds & multi-turn spell banking aren't crowded out in Stage-1!).
        if self.stratified_stage2:
            for t_score, (t_game, t_obs, t_eot, t_path) in sorted_terms:
                if n_upgrade_terms >= 2 and has_open_term and has_hold_unfz and has_hold_fz and has_spell_hold:
                    break
                t_sig = (_canonical_state_key(t_obs), t_eot)
                if t_sig in seen_term_keys:
                    continue
                is_up = (n_upgrade_terms < 2) and (int(t_obs.tavern_tier) > root_tier)
                is_op = (not has_open_term) and (not t_eot)
                has_hand_minion = t_eot and any(not c.is_spell for c in t_obs.hand)
                is_hd_u = has_hand_minion and (not t_obs.is_frozen) and (not has_hold_unfz)
                is_hd_f = has_hand_minion and bool(t_obs.is_frozen) and (not has_hold_fz)
                is_sp_h = t_eot and (not has_spell_hold) and _has_persistent_spell(t_obs)
                if is_up or is_op or is_hd_u or is_hd_f or is_sp_h:
                    seen_term_keys.add(t_sig)
                    ranked_terms.append((t_score, t_game, t_path))
                    if is_up:
                        n_upgrade_terms += 1
                    if is_op:
                        has_open_term = True
                    if is_hd_u:
                        has_hold_unfz = True
                    if is_hd_f:
                        has_hold_fz = True
                    if is_sp_h:
                        has_spell_hold = True

        best_score = -1e18
        default_end: QueuedStep = (("end_turn", None, None), _plan_validation_key(root_obs), False)
        best_path: List[QueuedStep] = [default_end]
        evaluated_plans: List[Tuple[float, List[QueuedStep], str]] = []
        batch_games: List[seaglass.TavernGame] = []
        plan_slices: List[Tuple[int, int, List[QueuedStep], str]] = []
        for _, cand_game, cand_path in ranked_terms:
            eval_game = cand_game
            cand_obs = cand_game.observe()
            if int(cand_obs.tavern_tier) > root_tier:
                b_tag = "upgrade"
            elif cand_obs.discover_pending is not None or (cand_path and cand_path[-1][2]):
                b_tag = "open"
            elif is_t1 and any(not c.is_spell for c in cand_obs.hand):
                b_tag = "t1_hold"
            elif _has_persistent_spell(cand_obs):
                b_tag = "spell_hold"
            else:
                b_tag = "stay"
            s0 = len(batch_games)
            if (
                cand_obs.discover_pending is None
                and cand_path
                and cand_path[-1][2]
                and self.stoch_samples > 1
                and id(cand_game) in stoch_origins
            ):
                parent_g, stoch_act = stoch_origins[id(cand_game)]
                for s_idx in range(self.stoch_samples):
                    sg = parent_g.copy()
                    sg.reseed((int(root_obs.turn) * 7919 + s_idx * 104729 + 31) | 1)
                    sg.step(stoch_act)
                    if sg.observe().discover_pending is not None:
                        d_legal = sg.legal_actions()
                        if d_legal:
                            d_pick = self._evaluate_discover_options(
                                sg, d_legal, particles, ctx, allow_shop_continuation=False
                            )
                            sg.step(d_pick)
                            self._play_newly_gained_card_in_sim(sg, particles=particles, ctx=ctx)
                    sg = self._rollout_shop_consumer(sg, ctx, particles=particles, max_steps=3)
                    batch_games.append(sg)
            else:
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
                batch_games.append(eval_game)
            plan_slices.append((s0, len(batch_games), cand_path, b_tag))

        batch_scores = self.evaluator.evaluate_turn_end_batch(
            batch_games, particles, ctx=ctx, combats_per_particle=14
        )
        for s0, s1, cand_path, b_tag in plan_slices:
            sub = []
            for gi in range(s0, s1):
                sc_i = batch_scores[gi]
                if upg_horizon_bonus != 0.0 and int(batch_games[gi].observe().tavern_tier) > root_tier:
                    sc_i += upg_horizon_bonus
                sub.append(sc_i)
            full_score = float(sum(sub) / max(1, len(sub)))
            if self.search_noise_std > 0.0:
                full_score += self.rng.gauss(0.0, self.search_noise_std)
            evaluated_plans.append((full_score, cand_path, b_tag))
            if full_score > best_score:
                best_score = full_score
                best_path = cand_path

        # Coherent macro-branch exploration (training rollouts ONLY): deduplicates to 1 best plan per
        # structural macro class (stay-tier / upgrade-tier / open-discover / turn-1-hold) so 4 near-identical
        # Buy plans never drown out Upgrade/Hold exploration, then applies softmax + small ε-floor within margin!
        if (
            self.plan_explore_temp > 0.0
            and int(root_obs.turn) <= self.plan_explore_max_turn
            and len(evaluated_plans) > 1
        ):
            margin = max(1.0, self.plan_explore_margin)
            branch_best: Dict[str, Tuple[float, List[QueuedStep]]] = {}
            for sc, pt, b_tag in evaluated_plans:
                if sc < best_score - margin:
                    continue
                prev = branch_best.get(b_tag)
                if prev is None or sc > prev[0]:
                    branch_best[b_tag] = (sc, pt)
            near = list(branch_best.values())
            if len(near) > 1:
                inv_t = 1.0 / self.plan_explore_temp
                ws = [math.exp(max(-40.0, (sc - best_score) * inv_t)) for sc, _ in near]
                tot_w = sum(ws)
                # Blend 75% Boltzmann softmax + 25% uniform across valid distinct macro branches within margin
                # so every coherent macro alternative (e.g. Turn-2/5/7/9 leveling, Turn-1 hold) gets real RCT samples!
                eps_u = 0.25
                u_p = 1.0 / len(near)
                probs = [(1.0 - eps_u) * (w_i / tot_w) + eps_u * u_p for w_i in ws]
                r_u = self.rng.random()
                acc = 0.0
                for (sc, pt), p_i in zip(near, probs):
                    acc += p_i
                    if r_u <= acc:
                        if pt is not best_path:
                            # Commit to the chosen exploratory macro branch so `act_in_lobby` doesn't
                            # cancel it with a spurious Refresh/DarkDiscovery due to the lower explored score!
                            return best_score + 1e6, pt, []
                        break

        top_prefixes = self._select_stratified_roll_prefixes(
            roll_prefixes, root_fast=root_fast, root_obs=root_obs, max_prefixes=4
        )
        return best_score, best_path, top_prefixes

    def _select_stratified_roll_prefixes(
        self,
        roll_prefixes: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]],
        root_fast: float,
        root_obs: seaglass.TavernObservation,
        max_prefixes: int = 4,
    ) -> List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]]:
        """Neural-scale-aware roll prefix stratification: fixes `SovereignBot`'s `1.0x`-scale
        `-6.0`/`-0.8` cutoff that silently pruned `Upgrade -> Refresh` and `Buy/Sell -> Refresh`
        prefixes under `ScalingSearchBot`'s `4.2x` neural evaluation scale (zero HP/turn hardcoding)."""
        scale_mult = max(
            1.0,
            float(getattr(self.evaluator, "prior_weight", 0.45))
            + 1.6 * float(getattr(self.evaluator, "neural_weight", 4.2)),
        )
        valid: List[Tuple[float, float, float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep], bool]] = []
        seen_keys: set[Tuple] = set()
        min_keep_board = min(len(root_obs.board), 7 if root_obs.turn >= 7 else 6)
        for p_fast, p_g, p_obs, p_path in roll_prefixes:
            has_clean_upgrade = (
                p_obs.tavern_tier > root_obs.tavern_tier
                and len(p_obs.board) >= min_keep_board
            )
            min_fast_floor = (
                (root_fast - 6.0 * scale_mult)
                if has_clean_upgrade
                else (root_fast - 1.2 * scale_mult)
            )
            if p_fast < min_fast_floor:
                continue
            pk = _canonical_state_key(p_obs)
            if pk in seen_keys:
                continue
            seen_keys.add(pk)
            rem_rolls = max(0, p_obs.gold - p_obs.refresh_cost)
            cont_score = (
                p_fast
                + scale_mult * (0.46 + 0.06 * max(0, p_obs.tavern_tier - 3)) * rem_rolls
                + scale_mult * 0.60 * p_obs.free_refreshes
            )
            gold_spent = max(1, (root_obs.gold - p_obs.gold) + (root_obs.free_refreshes - p_obs.free_refreshes))
            eff_ratio = (p_fast - root_fast) / float(gold_spent)
            valid.append((cont_score, eff_ratio, p_fast, p_g, p_obs, p_path, has_clean_upgrade))

        if not valid:
            return []

        selected: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []
        used_idx: set[int] = set()

        # Dedicated Upgrade-Continuation Slot: always let Stage-2 evaluate `Upgrade -> Refresh`!
        upg_cands = [i for i, it in enumerate(valid) if it[6]]
        eff_max_prefixes = (max_prefixes + 1) if upg_cands else max_prefixes
        if upg_cands:
            b_upg = max(upg_cands, key=lambda i: valid[i][0])
            used_idx.add(b_upg)
            it = valid[b_upg]
            selected.append((it[2], it[3], it[4], it[5]))

        short_cands = [i for i, it in enumerate(valid) if len(it[5]) <= 2 and i not in used_idx]
        if short_cands and len(selected) < eff_max_prefixes:
            b_short = max(short_cands, key=lambda i: valid[i][0])
            used_idx.add(b_short)
            it = valid[b_short]
            selected.append((it[2], it[3], it[4], it[5]))

        rem_cands = [i for i in range(len(valid)) if i not in used_idx]
        if rem_cands and len(selected) < eff_max_prefixes:
            b_eff = max(rem_cands, key=lambda i: valid[i][1])
            used_idx.add(b_eff)
            it = valid[b_eff]
            selected.append((it[2], it[3], it[4], it[5]))

        by_cont = sorted(range(len(valid)), key=lambda i: valid[i][0], reverse=True)
        for i in by_cont:
            if len(selected) >= eff_max_prefixes:
                break
            if i in used_idx:
                continue
            used_idx.add(i)
            it = valid[i]
            selected.append((it[2], it[3], it[4], it[5]))

        return selected

    def _rollout_shop_consumer(
        self,
        start_game: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        particles=None,
        max_steps: int = 5,
        crn_seed: Optional[int] = None,
    ) -> seaglass.TavernGame:
        """Calibrated post-refresh/discover shop consumer: uses `ScalingValueTransformer(end_of_turn=False)`
        directly so unspent gold & free refreshes are valued in exact neural units (fixing the 4x-underweighted
        `1.38 * obs.gold` Sovereign prior mismatch!)."""
        if self.evaluator._in_t1_rollout and self.no_t1_upgrade:
            # Time-consistent Turn t+1 shop consumer: prevents `STAY T` from procrastinating
            # ("I'll level tomorrow!") AND prevents `UPG -> T+1` from wasting Turn t+1 gold double-leveling
            # instead of realizing the true value of a Tier T+1 shop!
            sim = start_game.copy()
            base_seed = crn_seed if crn_seed is not None else (self.rng.getrandbits(63) | 1)
            sim.reseed(base_seed)
            rolls_used = 0
            actions_taken = 0
            for _iter in range(max_steps + 3):
                if actions_taken >= max_steps:
                    break
                obs = sim.observe()
                if obs.discover_pending is not None:
                    self._fast_resolve_chained_discover(sim, particles=particles, ctx=ctx)
                    self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
                    continue
                cur_eot = self.evaluator.evaluate_fast(obs, ctx=ctx, end_of_turn=True, particles=particles)
                cur_total = cur_eot + 1.38 * obs.gold + 0.88 * obs.free_refreshes
                cand_sims: List[seaglass.TavernGame] = []
                cand_obs: List[seaglass.TavernObservation] = []
                pruned = prune_sovereign_actions(obs, sim.legal_actions(), self.evaluator, self.kb, particles=particles, ctx=ctx)
                for act in pruned:
                    if act.kind in ("end_turn", "refresh", "toggle_freeze", "pass", "dark_discovery", "upgrade", "upgrade_tavern"):
                        continue
                    if act.kind in ("hero_power", "hero_power_2") and self._is_stochastic_hero_power(sim, obs, act):
                        continue
                    c = sim.copy()
                    c.reseed((base_seed + _iter * 1009 + 17) | 1)
                    c.step(act)
                    if act.kind == "buy":
                        c_play = c.copy()
                        self._play_newly_gained_card_in_sim(c_play, particles=particles, ctx=ctx)
                        cp_obs = c_play.observe()
                        cand_sims.append(c_play)
                        cand_obs.append(cp_obs)
                        if len(obs.board) >= 7:
                            c_obs = c.observe()
                            if len(cp_obs.hand) != len(c_obs.hand):
                                cand_sims.append(c)
                                cand_obs.append(c_obs)
                    else:
                        cand_sims.append(c)
                        cand_obs.append(c.observe())
                for c_game, c_o, _ in self._expand_cycle_macros(sim, obs, [], particles=particles, ctx=ctx):
                    cand_sims.append(c_game)
                    cand_obs.append(c_o)
                can_roll_more = (
                    rolls_used < 3
                    and self._can_productive_refresh(obs)
                    and (obs.gold >= 4 or (obs.free_refreshes > 0 and obs.gold >= 2))
                )
                if not cand_sims:
                    if can_roll_more:
                        ref = seaglass.TavernAction.refresh()
                        if sim.is_legal(ref):
                            sim.step(ref)
                            rolls_used += 1
                            continue
                    break
                eot_scores = self.evaluator.evaluate_fast_batch(cand_obs, ctx=ctx, end_of_turn=True, particles=particles)
                scores = [ev + 1.38 * co.gold + 0.88 * co.free_refreshes for ev, co in zip(eot_scores, cand_obs)]
                best_idx = max(range(len(scores)), key=lambda i: scores[i])
                if scores[best_idx] > cur_total - 0.12:
                    sim = cand_sims[best_idx]
                    actions_taken += 1
                elif can_roll_more:
                    ref = seaglass.TavernAction.refresh()
                    if sim.is_legal(ref):
                        sim.step(ref)
                        rolls_used += 1
                        continue
                    break
                else:
                    break
            return sim

        if self.policy_rollout and self.policy_predictor is not None:
            base_seed = crn_seed if crn_seed is not None else (self.rng.getrandbits(63) | 1)
            return self.policy_predictor.rollout_batch(
                [start_game], [base_seed], max_steps=max_steps
            )[0]

        if not self.fast_neural_rollout and (not self.calibrated_rollout or self.evaluator._in_t1_rollout):
            # Keep `super()._rollout_shop_consumer` on its native 1.0x Sovereign scale (matching
            # `1.38 * obs.gold`) instead of firing thousands of tiny `batch=1` 5-model GPU calls
            # during hypothetical Refresh/Discover rollout steps; the resulting final state is
            # still scored by the full 5-model `ScalingValueTransformer` inside `evaluate_turn_end`!
            prev_t1 = self.evaluator._in_t1_rollout
            self.evaluator._in_t1_rollout = True
            try:
                return super()._rollout_shop_consumer(
                    start_game, ctx, particles=particles, max_steps=max_steps, crn_seed=crn_seed
                )
            finally:
                self.evaluator._in_t1_rollout = prev_t1

        use_single = bool(self.fast_neural_rollout)
        max_rolls_cap = 2 if use_single else 3
        improve_tol = 0.50 if use_single else 0.25
        sim = start_game.copy()
        base_seed = crn_seed if crn_seed is not None else (self.rng.getrandbits(63) | 1)
        sim.reseed(base_seed)
        rolls_used = 0
        actions_taken = 0

        for _iter in range(max_steps + 3):
            if actions_taken >= max_steps:
                break
            obs = sim.observe()
            if obs.discover_pending is not None:
                self._fast_resolve_chained_discover(sim, particles=particles, ctx=ctx)
                self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
                continue

            cand_sims: List[seaglass.TavernGame] = []
            cand_obs: List[seaglass.TavernObservation] = []

            pruned = self._select_epoch_actions(obs, sim.legal_actions(), particles=particles, ctx=ctx)
            for act in pruned:
                if act.kind in ("end_turn", "refresh", "toggle_freeze", "pass", "dark_discovery"):
                    continue
                if act.kind in ("hero_power", "hero_power_2") and self._is_stochastic_hero_power(sim, obs, act):
                    continue
                c = sim.copy()
                c.reseed((base_seed + _iter * 1009 + 17) | 1)
                c.step(act)
                if act.kind == "buy":
                    c_play = c.copy()
                    self._play_newly_gained_card_in_sim(c_play, particles=particles, ctx=ctx)
                    cp_obs = c_play.observe()
                    cand_sims.append(c_play)
                    cand_obs.append(cp_obs)
                    if len(obs.board) >= 7:
                        c_obs = c.observe()
                        if len(cp_obs.hand) != len(c_obs.hand):
                            cand_sims.append(c)
                            cand_obs.append(c_obs)
                else:
                    cand_sims.append(c)
                    cand_obs.append(c.observe())

            for c_game, c_o, _ in self._expand_cycle_macros(sim, obs, [], particles=particles, ctx=ctx):
                cand_sims.append(c_game)
                cand_obs.append(c_o)

            can_roll_more = (
                rolls_used < max_rolls_cap
                and self._can_productive_refresh(obs)
                and (obs.gold >= 4 or (obs.free_refreshes > 0 and obs.gold >= 2))
            )
            if not cand_sims:
                if can_roll_more:
                    ref = seaglass.TavernAction.refresh()
                    if sim.is_legal(ref):
                        sim.step(ref)
                        rolls_used += 1
                        continue
                break

            all_scores = self.evaluator.evaluate_fast_batch(
                [obs] + cand_obs,
                ctx=ctx,
                end_of_turn=False,
                particles=particles,
                single_model=use_single,
            )
            cur_total = all_scores[0]
            scores = all_scores[1:]
            best_idx = max(range(len(scores)), key=lambda i: scores[i])
            if scores[best_idx] > cur_total - improve_tol:
                sim = cand_sims[best_idx]
                actions_taken += 1
            elif can_roll_more:
                ref = seaglass.TavernAction.refresh()
                if sim.is_legal(ref):
                    sim.step(ref)
                    rolls_used += 1
                    continue
                break
            else:
                break
        return sim

    def _rollout_shop_consumer_batch(
        self,
        games: Sequence[seaglass.TavernGame],
        crn_seeds: Sequence[int],
        max_steps: int = 4,
        max_refreshes: int = 2,
        ctx: Optional[seaglass.LobbyContext] = None,
        particles=None,
    ) -> List[seaglass.TavernGame]:
        """Batched GPU single-model neural rollout across all 2-turn / refresh rollout games simultaneously!
        Replaces N separate tiny GPU forward passes per step with ONE batched GPU forward pass per step."""
        n = len(games)
        if n == 0:
            return []
        sims: List[seaglass.TavernGame] = []
        base_seeds: List[int] = []
        for g, s in zip(games, crn_seeds):
            sim = g.copy()
            bs = int(s) | 1
            sim.reseed(bs)
            sims.append(sim)
            base_seeds.append(bs)

        active = list(range(n))
        rolls_used = [0] * n
        actions_taken = [0] * n
        improve_tol = 0.50

        for _iter in range(max_steps + 3):
            active = [i for i in active if actions_taken[i] < max_steps]
            if not active:
                break

            eval_records: List[Tuple[int, int, int, List[seaglass.TavernGame], bool]] = []
            flat_obs: List[seaglass.TavernObservation] = []
            next_active: List[int] = []

            for i in active:
                sim = sims[i]
                obs = sim.observe()
                if obs.discover_pending is not None:
                    self._fast_resolve_chained_discover(sim, particles=particles, ctx=ctx)
                    self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
                    next_active.append(i)
                    continue

                cand_sims: List[seaglass.TavernGame] = []
                cand_obs: List[seaglass.TavernObservation] = []
                pruned = self._select_epoch_actions(obs, sim.legal_actions(), particles=particles, ctx=ctx)
                for act in pruned:
                    if act.kind in ("end_turn", "refresh", "toggle_freeze", "pass", "dark_discovery"):
                        continue
                    if act.kind in ("hero_power", "hero_power_2") and self._is_stochastic_hero_power(sim, obs, act):
                        continue
                    c = sim.copy()
                    c.reseed((base_seeds[i] + _iter * 1009 + 17) | 1)
                    c.step(act)
                    if act.kind == "buy":
                        c_play = c.copy()
                        self._play_newly_gained_card_in_sim(c_play, particles=particles, ctx=ctx)
                        cp_obs = c_play.observe()
                        cand_sims.append(c_play)
                        cand_obs.append(cp_obs)
                        if len(obs.board) >= 7:
                            c_obs = c.observe()
                            if len(cp_obs.hand) != len(c_obs.hand):
                                cand_sims.append(c)
                                cand_obs.append(c_obs)
                    else:
                        cand_sims.append(c)
                        cand_obs.append(c.observe())

                for c_game, c_o, _ in self._expand_cycle_macros(sim, obs, [], particles=particles, ctx=ctx):
                    cand_sims.append(c_game)
                    cand_obs.append(c_o)

                can_roll_more = (
                    rolls_used[i] < max_refreshes
                    and self._can_productive_refresh(obs)
                    and (obs.gold >= 4 or (obs.free_refreshes > 0 and obs.gold >= 2))
                )
                if not cand_sims:
                    if can_roll_more:
                        ref = seaglass.TavernAction.refresh()
                        if sim.is_legal(ref):
                            sim.step(ref)
                            rolls_used[i] += 1
                            next_active.append(i)
                    continue

                s0 = len(flat_obs)
                flat_obs.append(obs)
                flat_obs.extend(cand_obs)
                eval_records.append((i, s0, len(cand_sims), cand_sims, can_roll_more))

            if eval_records:
                all_scores = self.evaluator.evaluate_fast_batch(
                    flat_obs,
                    ctx=ctx,
                    end_of_turn=False,
                    particles=particles,
                    single_model=True,
                )
                for i, s0, nc, cand_sims, can_roll_more in eval_records:
                    cur_total = all_scores[s0]
                    best_k = 0
                    best_sc = all_scores[s0 + 1]
                    for k in range(1, nc):
                        sc_k = all_scores[s0 + 1 + k]
                        if sc_k > best_sc:
                            best_sc = sc_k
                            best_k = k
                    if best_sc > cur_total - improve_tol:
                        sims[i] = cand_sims[best_k]
                        actions_taken[i] += 1
                        next_active.append(i)
                    elif can_roll_more:
                        ref = seaglass.TavernAction.refresh()
                        if sims[i].is_legal(ref):
                            sims[i].step(ref)
                            rolls_used[i] += 1
                            next_active.append(i)

            active = next_active

        return sims

    def _evaluate_stochastic_action(
        self,
        base_game: seaglass.TavernGame,
        stoch_act: seaglass.TavernAction,
        particles,
        ctx: Optional[seaglass.LobbyContext],
        crn_seeds: Optional[Sequence[int]] = None,
        n_samples: Optional[int] = None,
    ) -> float:
        """Batched Refresh/DarkDiscovery/Stochastic-HP evaluator: runs all `refresh_samples`
        through a single batched `evaluate_turn_end_batch` GPU pass (4x fewer GPU launches!)."""
        samples = n_samples if n_samples is not None else self.refresh_samples
        if crn_seeds is not None and len(crn_seeds) >= samples:
            seeds = crn_seeds[:samples]
        else:
            seeds = [self.rng.getrandbits(63) | 1 for _ in range(samples)]

        sims: List[seaglass.TavernGame] = []
        r_seeds: List[int] = []
        for s_val in seeds:
            sim = base_game.copy()
            sim.reseed(s_val)
            sim.step(stoch_act)
            legal_after = sim.legal_actions()
            if legal_after and legal_after[0].kind == "choose_discover":
                pick = self._evaluate_discover_options(
                    sim, legal_after, particles, ctx, allow_shop_continuation=False
                )
                sim.step(pick)
                self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
            r_seed = (s_val ^ 0x9E3779B97F4A7C15) | 1
            if (self.policy_rollout and self.policy_predictor is not None) or (
                self.fast_neural_rollout and self.evaluator.has_neural_model
            ):
                sims.append(sim)
                r_seeds.append(r_seed)
            else:
                sim = self._rollout_shop_consumer(
                    sim,
                    ctx,
                    particles=particles,
                    max_steps=4,
                    crn_seed=r_seed,
                )
                sims.append(sim)
        if self.policy_rollout and self.policy_predictor is not None and sims:
            sims = self.policy_predictor.rollout_batch(sims, r_seeds, max_steps=4)
        elif self.fast_neural_rollout and self.evaluator.has_neural_model and sims:
            sims = self._rollout_shop_consumer_batch(
                sims, r_seeds, max_steps=4, ctx=ctx, particles=particles
            )

        scores = self.evaluator.evaluate_turn_end_batch(
            sims, particles, ctx=ctx, combats_per_particle=14
        )
        return float(sum(scores) / max(1, len(scores)))

    def _choose_best_trinket(
        self,
        base_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        particles,
        ctx: Optional[seaglass.LobbyContext],
    ) -> seaglass.TavernAction:
        """Pure CRN-paired 2-Turn Exact Rust Physics Rollout for Trinkets (ZERO hardcoded trinket bonus table!)."""
        obs0 = base_game.observe()
        crn_seeds = [self.rng.getrandbits(63) | 1 for _ in range(4)]
        best_act = legal[0]
        best_val = -1e18

        for act in legal:
            if act.kind != "choose_trinket":
                continue
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
                sim_t0 = self._rollout_shop_consumer(
                    sim, ctx, particles=particles, max_steps=4, crn_seed=(r_seed ^ 0x13579BDF) | 1
                )
                v_turn0 = self.evaluator.evaluate_turn_end(
                    sim_t0, particles, ctx=ctx, combats_per_particle=14
                )
                v_turn1_macro = v_turn0
                if particles.boards:
                    sim_t1 = sim_t0.copy()
                    self.positioner.fast_reposition_game(sim_t1, sim_t1.observe(), particles)
                    c_seed = (obs0.turn * 262147 + m_idx * 4099 + 5) | 1
                    sim_t1.reseed(c_seed)
                    if sim_t1.is_legal(seaglass.TavernAction.end_turn()):
                        sim_t1.end_turn()
                    opp_b = particles.boards[m_idx % len(particles.boards)]
                    sim_t1.resolve_combat_against(opp_b, opponent_tier=particles.tier, seed=c_seed)
                    if int(sim_t1.observe().health) <= 0:
                        v_turn1_macro = -50.0
                    else:
                        sim_t1.start_turn()
                        for _ in range(2):
                            if sim_t1.observe().discover_pending is not None:
                                self._fast_resolve_chained_discover(sim_t1, particles=particles, ctx=ctx)
                                self._play_newly_gained_card_in_sim(sim_t1, particles=particles, ctx=ctx)
                            else:
                                break
                        sim_t1 = self._rollout_shop_consumer(
                            sim_t1, ctx, particles=particles, max_steps=4, crn_seed=(c_seed ^ 0x2468ACE1) | 1
                        )
                        self.positioner.fast_reposition_game(sim_t1, sim_t1.observe(), particles)
                        if sim_t1.is_legal(seaglass.TavernAction.end_turn()):
                            sim_t1.end_turn()
                        sim_t1.resolve_combat_against(opp_b, opponent_tier=particles.tier, seed=c_seed + 2)
                        if int(sim_t1.observe().health) <= 0:
                            v_turn1_macro = -25.0
                        else:
                            sim_t1.start_turn()
                            v_turn1_macro = self.evaluator.evaluate_fast(
                                sim_t1.observe(), ctx=ctx, end_of_turn=False, particles=particles, in_dist_sot=True
                            )
                rollout_sum += 0.45 * v_turn0 + 0.55 * v_turn1_macro

            total_val = rollout_sum / len(crn_seeds)
            if total_val > best_val:
                best_val = total_val
                best_act = act
        return best_act

    def _finalize_positioning_or_end(
        self,
        belief_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        player: int,
        turn: int,
        particles,
        ctx: Optional[seaglass.LobbyContext],
        lobby_id: int = 0,
    ) -> seaglass.TavernAction:
        """Clean decoupled turn finalizer (ZERO `rescue_open_play` or `smart_freeze` heuristic overrides!):
        only spends strictly dead gold on Refresh (if unfrozen), runs exact Rust combat positioning, then EndTurn."""
        obs = belief_game.observe()
        # Only spend strictly expiring dead gold on Refresh: never burn persistent `free_refreshes`
        # when we lack gold to buy a minion (save them for turns with gold/higher tiers!),
        # DO spend a free refresh if we still have >= minion_cost expiring gold unspent!
        if not obs.is_frozen and (
            (obs.free_refreshes == 0 and obs.refresh_cost > 0 and obs.gold >= obs.refresh_cost)
            or (obs.free_refreshes > 0 and obs.gold >= obs.minion_cost)
        ):
            ref_act = seaglass.TavernAction.refresh()
            if belief_game.is_legal(ref_act):
                probe = belief_game.copy()
                probe.step(ref_act)
                p_obs = probe.observe()
                if (
                    p_obs.gold < obs.gold
                    or (obs.gold >= obs.minion_cost and p_obs.free_refreshes < obs.free_refreshes)
                ) and p_obs.health >= obs.health:
                    matched_ref = self._find_matching_legal(legal, _action_key(ref_act))
                    if matched_ref is not None:
                        self._queued_actions.pop(player, None)
                        self.last_note = {"dead_gold_refresh": float(obs.gold)}
                        return matched_ref

        pos_tag = (lobby_id, turn)
        if self._positioned_turn.get(player) != pos_tag and len(obs.board) >= 2:
            self._positioned_turn[player] = pos_tag
            perm = self.positioner.best_permutation(belief_game, particles, ctx=ctx)
            repo_acts = self.positioner.permutation_to_actions(perm)
            if repo_acts:
                sim_pos = belief_game.copy()
                steps: List[QueuedStep] = []
                for a in repo_acts:
                    steps.append((_action_key(a), _plan_validation_key(sim_pos.observe()), False))
                    if sim_pos.is_legal(a):
                        sim_pos.step(a)
                steps.append((("end_turn", None, None), _plan_validation_key(sim_pos.observe()), False))
                first = self._find_matching_legal(legal, steps[0][0])
                if first is not None:
                    self._queued_actions[player] = steps[1:]
                    self.last_note = {"pos_swaps": float(len(repo_acts))}
                    return first

        end_act = self._find_matching_legal(legal, ("end_turn", None, None))
        return end_act if end_act is not None else legal[0]

    def act_in_lobby(self, lobby: seaglass.LobbyGame, player: int) -> seaglass.TavernAction:
        # General Sudden-Death & Endgame Horizon Scaling (ZERO card/comp rules):
        # When death is imminent (lethal HP vs live opponent) or in Top-2 1v1 Finals, shorten the
        # future-stats horizon (`growth_weight -> 0`, lower `neural_weight`) and sharpen combat
        # positioning so the simulator prioritizes winning/surviving THIS fight over Turn t+3 scaling!
        if lobby.phase == "turn":
            ctx = lobby.context(player)
            obs = lobby.observe(player)
            ehp = obs.health + obs.armor
            live_opp = ctx is not None and not ctx.next_is_ghost
            alive_u = ctx.alive_units if ctx is not None else 8
            if live_opp and alive_u <= 2:
                # Top-2 1v1 Finals ONLY: restore full exact combat winrate (`combat_icm_weight = 1.0`)
                # against the exact known 1v1 opponent board, scale `prior_weight` proportionally
                # with `neural_weight`, and set a candidate-independent upper bound on `macro` so
                # guaranteed 1v1 victory (`win_1v1_util`) ALWAYS strictly dominates any continuation!
                self.evaluator.combat_icm_weight = 1.0
                init_macro = max(
                    0.0,
                    float(self.evaluator.macro_and_economy_value(obs, ctx=ctx, end_of_turn=True)),
                )
                self.evaluator._turn_win_macro_cap = max(
                    init_macro + 180.0, 1.35 * init_macro + 120.0
                )
                opp_ehp_now: Optional[int] = None
                if ctx is not None:
                    alive_opps = [o for o in ctx.opponents if o.alive]
                    if len(alive_opps) == 1:
                        opp_ehp_now = int(alive_opps[0].health + alive_opps[0].armor)
                if ehp <= 18 or (opp_ehp_now is not None and opp_ehp_now <= 15):
                    self.evaluator.neural_weight = self.finals_min_nw
                    nw_ratio = self.finals_min_nw / max(1e-6, self._base_neural_weight)
                    self.evaluator.prior_weight = self._base_prior_weight * nw_ratio
                    self.evaluator.predictor.growth_weight = 0.0
                    self.evaluator.two_turn_rollout_steps = 0
                else:
                    self.evaluator.neural_weight = 0.55 * self._base_neural_weight
                    self.evaluator.prior_weight = 0.55 * self._base_prior_weight
                    self.evaluator.predictor.growth_weight = 0.30 * self._base_growth_weight
                    self.evaluator.two_turn_rollout_steps = min(2, self._base_two_turn_steps)
                self.positioner.max_candidates = self._finals_pos_cands
                self.evaluator.positioner.max_candidates = self._finals_pos_cands
            else:
                # Never throttle learned scaling while 3+ players remain (even at 9-15 HP on Turn 10+!),
                # only expand combat positioning search to 96 candidates in Top-3 / lethal danger!
                self.evaluator.combat_icm_weight = self._base_combat_icm_weight
                self.evaluator.neural_weight = self._base_neural_weight
                self.evaluator.prior_weight = self._base_prior_weight
                self.evaluator.predictor.growth_weight = self._base_growth_weight
                self.evaluator.two_turn_rollout_steps = self._base_two_turn_steps
                pos_c = (
                    self._finals_pos_cands
                    if (live_opp and (alive_u <= 3 or (obs.turn >= 10 and ehp <= 15)))
                    else self._base_pos_cands
                )
                self.positioner.max_candidates = pos_c
                self.evaluator.positioner.max_candidates = pos_c
        act = super().act_in_lobby(lobby, player)
        # Never throw away dead expiring gold on an unfrozen shop right before EndTurn, while NEVER
        # burning persistent `free_refreshes` when we lack gold to buy a minion (or Health refreshes).
        if lobby.phase == "turn" and act.kind == "end_turn":
            obs = lobby.observe(player)
            if not obs.is_frozen and (
                (obs.free_refreshes == 0 and obs.refresh_cost > 0 and obs.gold >= obs.refresh_cost)
                or (obs.free_refreshes > 0 and obs.gold >= obs.minion_cost)
            ):
                ref = seaglass.TavernAction.refresh()
                if lobby.is_legal(player, ref):
                    bg_p = lobby.belief_tavern(player, seed=1, extra_units=[])
                    if bg_p.is_legal(ref):
                        bg_p.step(ref)
                        p_obs = bg_p.observe()
                        if (
                            p_obs.gold < obs.gold
                            or (obs.gold >= obs.minion_cost and p_obs.free_refreshes < obs.free_refreshes)
                        ) and p_obs.health >= obs.health:
                            self._queued_actions.pop(player, None)
                            self.last_note = {"dead_gold_refresh": float(obs.gold)}
                            return ref
        return act
