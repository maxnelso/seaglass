"""ZeroPhysicsBot: Pure Rust-Physics + Learned Set-Transformer Battlegrounds Bot.

Zero hardcoded card tier lists, zero hardcoded hero/trinket tables, zero hardcoded
turn-by-turn leveling scripts, zero `SovereignBot` inheritance!

Core Pillars:
  1. Exact Rust Forward Physics (`end_turn()` + `resolve_combat_against()` + `start_turn()`
     + `equity_against()`) directly measures per-turn permanent stat growth, EOT card
     generation, persistent aura velocity, and combat win/lethal probability for ALL
     252 minions, 123 heroes, 243 trinkets, and 47 Dark Gifts automatically.
  2. Learned Set-Transformer Value Network (`ZeroValueNet` / `ZeroNumpyPredictor`) trained
     via AlphaZero-style Iterated Self-Play on RTX 3070 over complete engine states +
     physics velocity features.
  3. Clean General Beam Search + CRN-Paired Expectimax (`Refresh`, `DarkDiscovery`,
     `ChooseTrinket`, `ChooseDiscover`, `ChooseHero`, `HeroPower`) + Pure-Physics
     End-of-Turn Combat Positioning.
"""

from __future__ import annotations

import math
import os
import random
from typing import Any, Dict, List, Optional, Sequence, Tuple

import numpy as np
import seaglass

from .zero_model import (
    DEFAULT_ZERO_NPZ_PATH,
    ZERO_PHYSICS_DIM,
    ZeroFeaturizer,
    ZeroNumpyPredictor,
    get_zero_vocab,
)


def _action_key(act: seaglass.TavernAction) -> Tuple[str, Any, Any]:
    return (act.kind, act.args[0], act.args[1])


def _state_sig(obs: seaglass.TavernObservation) -> Tuple:
    return (
        obs.turn,
        obs.tavern_tier,
        obs.gold,
        obs.upgrade_cost,
        obs.is_frozen,
        obs.free_refreshes,
        obs.dark_discoveries_left,
        obs.hero_power_usable,
        tuple((u.card_id, u.attack, u.health, u.is_golden, u.activated_this_turn, u.charges) for u in obs.board),
        tuple((c.card_id, c.is_golden, c.attack, c.health, c.spell_cost) for c in obs.hand),
        tuple((s.card_id, s.attack, s.health) for s in obs.shop),
        obs.discover_pending is not None,
    )


def _canon_key(obs: seaglass.TavernObservation) -> Tuple:
    return (
        obs.tavern_tier,
        obs.gold,
        obs.upgrade_cost,
        obs.is_frozen,
        obs.dark_discoveries_left,
        obs.hero_power_usable,
        tuple(sorted((u.card_id, u.attack, u.health, u.is_golden, u.activated_this_turn, u.charges, u.taunt, u.divine_shield, u.reborn, u.venomous) for u in obs.board)),
        tuple(sorted((c.card_id, c.is_golden, c.attack, c.health, c.spell_cost) for c in obs.hand)),
        len(obs.shop),
        obs.discover_pending is not None,
    )


def _total_stats(obs: seaglass.TavernObservation) -> int:
    return sum(max(0, u.attack) + max(0, u.health) for u in obs.board)


def _sum_positive_counters(obs: seaglass.TavernObservation) -> float:
    s = 0.0
    for v in obs.player_counters:
        if v > 0:
            s += float(v)
    return s


class ZeroPhysicsBot:
    """
    Pure Rust-Physics + Learned Set-Transformer Search Bot (AlphaZero-Physics).
    Works cleanly in Gen-0 (pure physics bootstrap, zero neural weights) AND Gen-1+
    (learned `ZeroValueNet` weights driving long-horizon 1st-place utility).
    """

    def __init__(
        self,
        weights_path: Optional[str] = None,
        beam_width: int = 24,
        max_depth: int = 8,
        top_k_physics: int = 8,
        refresh_samples: int = 6,
        combats_eval: int = 12,
        neural_alpha: float = 0.68,
        two_turn_lookahead: bool = True,
        explore_noise: float = 0.0,
        seed: int = 2029,
        **kwargs,
    ) -> None:
        self.vocab = get_zero_vocab()
        self.featurizer = ZeroFeaturizer()
        w_path = weights_path if weights_path is not None else DEFAULT_ZERO_NPZ_PATH
        self.predictor = ZeroNumpyPredictor(w_path) if (w_path and os.path.exists(w_path)) else None
        self.beam_width = beam_width
        self.max_depth = max_depth
        self.top_k_physics = top_k_physics
        self.refresh_samples = refresh_samples
        self.combats_eval = combats_eval
        self.neural_alpha = neural_alpha
        self.two_turn_lookahead = two_turn_lookahead
        self.explore_noise = explore_noise
        self.rng = random.Random(seed)
        self.last_note: Optional[Dict[str, float]] = None

        self._hook_map = self.vocab.card_hook_feats
        self._queued: Dict[int, List[Tuple[Tuple[str, Any, Any], Tuple, bool]]] = {}
        self._pos_done: Dict[int, Tuple[int, int]] = {}
        self._ref_count: Dict[Tuple[int, int, int], int] = {}
        # Cache last turn's physics vector for clean self-play trajectory logging!
        self.last_physics_vec: Optional[List[float]] = None

    @property
    def has_net(self) -> bool:
        return self.predictor is not None and self.predictor.is_ready

    # -------------------------------------------------------------------------
    # Generic Opponent Board Belief (100% Pure, Zero Card Hardcoding!)
    # -------------------------------------------------------------------------
    def _opponent_board_and_tier(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext],
    ) -> Tuple[List[seaglass.Unit], int, bool, int]:
        """Returns `(opp_board, opp_tier, is_ghost, opp_max_hp)`."""
        turn = max(1, int(obs.turn))
        is_ghost = bool(ctx.next_is_ghost) if ctx is not None else False
        opp_tier = min(6, max(1, (turn + 1) // 2))
        if ctx is not None and ctx.opponents:
            for opp in ctx.opponents:
                if opp.is_next:
                    opp_tier = max(1, int(opp.tier))
                    if opp.seen_board:
                        dt = 0 if is_ghost else max(0, turn - (opp.seen_turn or turn))
                        if dt == 0:
                            b = list(opp.seen_board)
                        else:
                            # Gentle proportional growth over `dt` unseen turns (zero card hardcoding!)
                            scale = min(2.4, 1.0 + 0.16 * dt)
                            b = []
                            for u in opp.seen_board:
                                na = max(1, int(round(u.attack * scale)))
                                nh = max(1, int(round(u.health * scale)))
                                if u.venomous or (u.attack <= 6 and u.health <= 6 and turn >= 10):
                                    b.append(u)
                                else:
                                    toks = [f"{na}/{nh}"]
                                    if u.taunt:
                                        toks.append("taunt")
                                    if u.divine_shield:
                                        toks.append("divine_shield")
                                    if u.windfury:
                                        toks.append("windfury")
                                    if u.reborn:
                                        toks.append("reborn")
                                    try:
                                        b.append(seaglass.parse_unit(" ".join(toks)))
                                    except Exception:
                                        b.append(u)
                        max_hp = max((u.health for u in b), default=0)
                        return b, opp_tier, is_ghost, max_hp
                    break

        # Synthetic generic turn-calibrated opponent board when unseen
        n_u = min(7, 1 if turn == 1 else (2 if turn <= 3 else (4 if turn == 4 else (5 if turn == 5 else (6 if turn == 6 else 7)))))
        if turn <= 3:
            st = 2 + turn
        elif turn <= 6:
            st = 4 + (turn - 2) * 3
        elif turn <= 10:
            st = 16 + (turn - 6) * 10
        else:
            st = 56 + int(((turn - 10) ** 1.55) * 20)
        b = []
        for i in range(n_u):
            s_i = max(2, int(st * (1.25 if i < 2 else 0.85)))
            tok = f"{s_i}/{s_i}" + (" divine_shield" if (turn >= 9 and i == 0) else "")
            b.append(seaglass.parse_unit(tok))
        return b, opp_tier, is_ghost, max((u.health for u in b), default=0)

    # -------------------------------------------------------------------------
    # Fast Generic Board Order (100% Engine Hook Tags, Zero Card Names!)
    # -------------------------------------------------------------------------
    def _fast_generic_order(self, g: seaglass.TavernGame) -> None:
        obs = g.observe()
        board = obs.board
        n = len(board)
        if n <= 1:
            return
        # Score each unit's desired left-to-right priority strictly from engine tags!
        keys: List[Tuple[float, int]] = []
        for i, u in enumerate(board):
            hk = self._hook_map.get(int(u.card_id), (0.0, 0.0, 0.0, 0.0, 0.0, 0.0))
            is_bc, is_dr, is_eot, is_rally, is_cleave, is_avenge = hk
            st = u.attack + u.health
            # Lower key = further LEFT (attacks earlier)
            if is_cleave > 0.5 or is_rally > 0.5:
                k = -100.0 - st * 0.01
            elif u.divine_shield and u.attack >= 15:
                k = -40.0 - st * 0.01
            elif (is_avenge > 0.5 or (is_eot > 0.5 and st < 30)) and not u.taunt:
                k = 80.0 + st * 0.01
            elif u.taunt:
                k = 50.0  # Push taunts toward right edge to limit cleave splash
            else:
                k = -st * 0.01
            keys.append((k, i))
        target = [idx for _, idx in sorted(keys)]
        cur = list(range(n))
        for dst, orig_i in enumerate(target):
            src = cur.index(orig_i)
            if src != dst:
                act = seaglass.TavernAction.reposition(src, dst)
                if g.is_legal(act):
                    g.step(act)
                cur.insert(dst, cur.pop(src))

    # -------------------------------------------------------------------------
    # Exact Rust Forward-Physics Probe (Zero Hardcoding!)
    # -------------------------------------------------------------------------
    def compute_physics_probe(
        self,
        g: seaglass.TavernGame,
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        is_ghost: bool,
        combats: int = 10,
        seed: int = 1337,
    ) -> Tuple[List[float], Dict[str, float]]:
        """
        Simulates exact Rust `end_turn()` -> `equity_against()` -> `resolve_combat_against()` -> `start_turn()`
        to directly measure per-turn EOT growth, permanent combat growth, aura velocity, and SOT economy!
        """
        o0 = g.observe()
        turn = max(1, int(o0.turn))
        s0 = _total_stats(o0)
        c0 = _sum_positive_counters(o0)
        h0 = len(o0.hand)
        ehp = max(1, int(o0.health + o0.armor))

        sim = g.copy()
        sim.reseed(seed | 1)
        # Drain any pending discover/trinket defensively before end_turn
        for _ in range(2):
            so = sim.observe()
            if so.discover_pending is not None or so.trinket_options:
                la = sim.legal_actions()
                if la:
                    sim.step(la[0])
            else:
                break
        self._fast_generic_order(sim)
        if sim.is_legal(seaglass.TavernAction.end_turn()):
            sim.end_turn()
        o_eot = sim.observe()
        s_eot = _total_stats(o_eot)
        c_eot = _sum_positive_counters(o_eot)
        eot_stat_delta = float(max(0, s_eot - s0))
        eot_hand_delta = float(max(0, len(o_eot.hand) - h0))
        eot_counter_delta = float(max(0.0, c_eot - c0))

        # Combat distribution
        if is_ghost or not opp_board:
            win_r, draw_r, dmg_mean, lethal_p = 1.0, 0.0, 0.0, 0.0
        else:
            dist = sim.equity_against(
                opp_board,
                opponent_tier=opp_tier,
                n=max(4, combats),
                base_seed=(seed ^ 0x51A7) | 1,
            )
            win_r = float(dist.a_win_rate)
            draw_r = float(dist.draw_rate)
            dmg_mean = float(dist.damage_mean)
            # Estimate lethal risk smoothly from loss rate + expected damage taken
            loss_r = max(0.0, 1.0 - win_r - draw_r)
            est_dmg_on_loss = max(0.0, -dmg_mean / max(0.05, loss_r)) if dmg_mean < 0 else 0.0
            if ehp <= 15 and loss_r > 0.05 and est_dmg_on_loss >= ehp * 0.75:
                lethal_p = loss_r * min(1.0, est_dmg_on_loss / max(1.0, float(ehp)))
            else:
                lethal_p = 0.0

        # Single exact combat + start_turn() to measure permanent combat scaling & SOT economy!
        perm_combat_delta = 0.0
        sot_gold = float(min(10, o0.max_gold + 1) + o0.bonus_gold_next_turn)
        sot_hand_delta = 0.0
        if opp_board and not is_ghost:
            sim.resolve_combat_against(opp_board, opponent_tier=opp_tier, seed=(seed ^ 0x9E37) | 1)
            if sim.observe().health > 0:
                sim.start_turn()
                o_sot = sim.observe()
                perm_combat_delta = float(max(0, _total_stats(o_sot) - s_eot))
                sot_gold = float(o_sot.gold)
                sot_hand_delta = float(max(0, len(o_sot.hand) - len(o_eot.hand)))
        else:
            # Ghost turn: still advance start_turn() to measure SOT trinket/gold economy!
            if opp_board:
                sim.resolve_combat_against(opp_board, opponent_tier=opp_tier, seed=(seed ^ 0x9E37) | 1)
            if sim.observe().health > 0:
                sim.start_turn()
                o_sot = sim.observe()
                perm_combat_delta = float(max(0, _total_stats(o_sot) - s_eot))
                sot_gold = float(o_sot.gold)
                sot_hand_delta = float(max(0, len(o_sot.hand) - len(o_eot.hand)))

        horizon = float(max(0, 16 - turn))
        pvec = [
            win_r,
            draw_r,
            max(-2.0, min(2.0, dmg_mean / 15.0)),
            lethal_p,
            math.log1p(eot_stat_delta) / 8.0,
            math.log1p(perm_combat_delta) / 8.0,
            min(2.0, eot_hand_delta / 5.0),
            min(2.0, sot_gold / 15.0),
            min(2.0, sot_hand_delta / 5.0),
            math.log1p(eot_counter_delta) / 5.0,
            math.log1p(eot_stat_delta + perm_combat_delta) * horizon / 40.0,
        ]
        raw = {
            "win_r": win_r,
            "draw_r": draw_r,
            "dmg_mean": dmg_mean,
            "lethal_p": lethal_p,
            "eot_stat": eot_stat_delta,
            "perm_combat": perm_combat_delta,
            "eot_hand": eot_hand_delta,
            "sot_gold": sot_gold,
            "sot_hand": sot_hand_delta,
            "eot_ctr": eot_counter_delta,
        }
        return pvec, raw

    # -------------------------------------------------------------------------
    # Pure-Physics Bootstrap Value Function (ZERO Card Names!)
    # -------------------------------------------------------------------------
    def _pure_physics_value(
        self,
        obs: seaglass.TavernObservation,
        raw: Optional[Dict[str, float]],
        ctx: Optional[seaglass.LobbyContext],
        opp_max_hp: int,
        end_of_turn: bool = True,
    ) -> float:
        turn = max(1, int(obs.turn))
        tier = int(obs.tavern_tier)
        ehp = float(obs.health + obs.armor)
        alive_units = ctx.alive_units if ctx is not None else 8
        is_ghost = bool(ctx.next_is_ghost) if ctx is not None else False
        is_finals = alive_units <= 2 and not is_ghost
        horizon = 0.0 if is_finals else max(1.0, 15.5 - float(turn))

        board = obs.board
        hand = obs.hand
        cur_stats = float(_total_stats(obs))

        # 1. Survival & Combat Equity
        score = 0.0
        win_r = 0.5
        if raw is not None:
            win_r = raw["win_r"]
            draw_r = raw["draw_r"]
            dmg_m = raw["dmg_mean"]
            lethal_p = raw["lethal_p"]
            if is_finals:
                score += 125.0 * win_r + 36.0 * draw_r + 1.5 * dmg_m
            else:
                hp_urg = 1.0 + max(0.0, (28.0 - ehp) / 12.0)
                score += (30.0 * win_r + 11.0 * draw_r + 1.45 * dmg_m) * hp_urg - 210.0 * lethal_p

            # 2. Compound Multi-Turn Growth Projection (purely from Rust physics!)
            #    Power-law (`x ** 0.44`) stays hungry for massive scaling at 1k, 10k, AND 100k stats!
            per_turn = (
                raw["eot_stat"]
                + 0.95 * raw["perm_combat"]
                + 3.2 * raw["eot_ctr"] * max(1.0, float(len(board)))
            )
            proj_stats = cur_stats + horizon * per_turn + 0.16 * (horizon * horizon) * (raw["eot_ctr"] * 4.5)
            score += 4.5 * (max(0.0, proj_stats) ** 0.44)
            if not is_finals:
                score += 2.8 * (max(0.0, per_turn) ** 0.44) * min(6.0, horizon)
                score += 2.2 * raw["sot_gold"] + 4.2 * raw["eot_hand"] + 3.6 * raw["sot_hand"]
        else:
            # Fast mid-layer approximation (no physics probe yet)
            score += 4.2 * (max(0.0, cur_stats) ** 0.44)
            for u in board:
                hk = self._hook_map.get(int(u.card_id), (0.0, 0.0, 0.0, 0.0, 0.0, 0.0))
                score += 1.6 * u.tavern_tier + (2.8 if hk[2] > 0.5 else 0.0) + (2.5 if hk[3] > 0.5 else 0.0)

        # 3. Generic Keyword, Synergy & Scam Value (100% engine-native, ZERO card names!)
        tribe_counts: Dict[str, int] = {}
        copies: Dict[int, int] = {}
        for u in board:
            if u.divine_shield:
                score += 2.5 + 0.45 * (max(0.0, float(u.attack)) ** 0.45)
            if u.reborn:
                score += 2.0
            if u.windfury:
                score += 1.5 + 0.40 * (max(0.0, float(u.attack)) ** 0.45)
            if u.is_golden:
                score += 2.5 + 0.6 * u.tavern_tier
            if u.venomous:
                scam_scale = 38.0 if (is_finals or opp_max_hp >= 160) else 7.5
                score += scam_scale
            if u.dark_gifts:
                score += 3.5 * len(u.dark_gifts)
            if not u.is_golden:
                copies[u.card_id] = copies.get(u.card_id, 0) + 1
            for tr in u.tribes:
                tribe_counts[tr] = tribe_counts.get(tr, 0) + 1

        # Generic tribal focus bonus (pure counting)
        max_tribe = max((v for k, v in tribe_counts.items() if k != "all"), default=0)
        if max_tribe >= 3 and not is_finals:
            score += 1.8 * (max_tribe - 2)

        # Board fill penalty if ending turn with open slots (prevents naked-board P8 suicide!)
        want_board = min(7, turn + 1) if turn <= 5 else 7
        if end_of_turn and len(board) < want_board:
            playable_in_hand = any(not c.is_spell and not c.unplayable for c in hand)
            if playable_in_hand:
                score -= 26.0
            elif turn >= 3:
                score -= (6.5 if turn <= 5 else 10.5) * (want_board - len(board))

        # 4. Macro Horizon Value: Tavern Tier, DarkDiscovery Option Value, Hand & Gold!
        if not is_finals:
            # Smooth future value of higher Tavern Tiers over remaining turns (calibrated so it reaches Tier 5/6!)
            safety = 1.05 if is_ghost else max(0.28, min(1.10, (ehp - 6.0) / 20.0))
            tier_val = (tier ** 1.38) * 1.48 * min(9.5, horizon) * safety
            score += tier_val

            # `DarkDiscovery` option value (pure Patch 37.0.2 game mechanics: Turn 9 unlocks T4/5/6,
            # Turn 10+ is T5/6 ONLY with FirstTwo guaranteed tribe match!). Drops smoothly if low HP!
            dd_left = int(obs.dark_discoveries_left)
            if dd_left > 0:
                dd_hp_gate = 1.0 if (is_ghost or ehp >= 22.0) else max(0.15, (ehp - 8.0) / 14.0)
                if turn <= 6:
                    score += 36.0 * dd_left * dd_hp_gate
                elif turn == 7:
                    score += 32.0 * dd_left * dd_hp_gate
                elif turn == 8:
                    score += 24.0 * dd_left * dd_hp_gate
                elif turn == 9:
                    score += 10.0 * dd_left * dd_hp_gate
                else:
                    score += 1.5 * dd_left

            # Persistent engine counters (all 53 counters, zero card names!)
            ctr_sum = _sum_positive_counters(obs)
            if ctr_sum > 0:
                score += 3.2 * (ctr_sum ** 0.50) * min(6.0, horizon)

            # Hand cards & Pair/Triple potential
            for c in hand:
                if c.is_spell:
                    score += 2.8 if not end_of_turn else 1.2
                else:
                    if not c.is_golden:
                        copies[c.card_id] = copies.get(c.card_id, 0) + 1
                    score += (10.0 + 2.0 * tier) if c.is_golden else (2.4 + 0.5 * c.tavern_tier)
            for cnt in copies.values():
                if cnt == 2:
                    score += 3.6 + 0.4 * tier

            # Frozen shop bonus ONLY if shop contains a Triple!
            if obs.is_frozen and obs.shop:
                has_trip = any(not s.is_spell and not s.is_golden and copies.get(s.card_id, 0) >= 2 for s in obs.shop)
                score += (5.5 + 0.5 * tier) if has_trip else -1.8

        # Remaining gold (valuable mid-turn, small dead-gold value at EOT)
        score += (2.4 if not end_of_turn else 0.15) * obs.gold + (1.2 if not end_of_turn else 0.3) * obs.free_refreshes
        score += 1.6 * obs.bonus_gold_next_turn
        if obs.discover_pending is not None:
            score += 9.0 + 1.5 * tier

        return score

    # -------------------------------------------------------------------------
    # Blended Pure-Physics + Learned Set-Transformer Evaluator
    # -------------------------------------------------------------------------
    def _evaluate_games_batch(
        self,
        games: Sequence[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        is_ghost: bool,
        opp_max_hp: int,
        combats: int = 8,
        seed_base: int = 1001,
    ) -> Tuple[List[float], List[List[float]]]:
        obs_list = [g.observe() for g in games]
        pvecs: List[List[float]] = []
        phys_scores: List[float] = []
        for i, g in enumerate(games):
            pv, raw = self.compute_physics_probe(
                g, opp_board, opp_tier, is_ghost, combats=combats, seed=seed_base + i * 31
            )
            pvecs.append(pv)
            phys_scores.append(
                self._pure_physics_value(obs_list[i], raw, ctx, opp_max_hp, end_of_turn=True)
            )

        if self.has_net and self.predictor is not None:
            net_util, _, pred_fp = self.predictor.predict_batch_full(
                obs_list, ctx=ctx, physics_list=pvecs, end_of_turn=True
            )
            alive_units = ctx.alive_units if ctx is not None else 8
            is_finals = alive_units <= 2 and not is_ghost
            # In Top-2 Finals, keep high weight on exact Rust combat winrate!
            alpha = 0.25 if is_finals else self.neural_alpha
            turn_i = int(obs_list[0].turn) if obs_list else 1
            fp_w = 0.0 if (is_finals or turn_i >= 14) else (2.4 * alpha)
            blended = [
                (1.0 - alpha) * ps + alpha * (2.0 * float(nu)) + fp_w * float(fp)
                for ps, nu, fp in zip(phys_scores, net_util, pred_fp)
            ]
            return blended, pvecs
        return phys_scores, pvecs

    def _fast_score_obs_batch(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        ctx: Optional[seaglass.LobbyContext],
        opp_max_hp: int,
        end_of_turn: bool = False,
    ) -> List[float]:
        phys = [
            self._pure_physics_value(o, None, ctx, opp_max_hp, end_of_turn=end_of_turn)
            for o in obs_list
        ]
        if self.has_net and self.predictor is not None:
            net_util, _ = self.predictor.predict_batch(
                obs_list, ctx=ctx, physics_list=None, end_of_turn=end_of_turn
            )
            a = 0.55 * self.neural_alpha
            return [(1.0 - a) * ps + a * (1.5 * float(nu)) for ps, nu in zip(phys, net_util)]
        return phys

    # -------------------------------------------------------------------------
    # Generic Legal Action Filter (Zero Card Hardcoding!)
    # -------------------------------------------------------------------------
    def _filter_epoch_actions(
        self,
        obs: seaglass.TavernObservation,
        legal: Sequence[seaglass.TavernAction],
    ) -> List[seaglass.TavernAction]:
        out: List[seaglass.TavernAction] = []
        n_board = len(obs.board)
        seen_play_pos: set[Tuple[int, bool]] = set()
        seen_hand_sym: set[Tuple] = set()
        canon_hand: set[int] = set()
        for hi, hc in enumerate(obs.hand):
            sk = (hc.card_id, hc.is_golden, hc.attack, hc.health, hc.spell_cost, hc.unplayable)
            if sk not in seen_hand_sym:
                seen_hand_sym.add(sk)
                canon_hand.add(hi)

        # Weakest 2 board slots by raw stats/tier for generic sell-cycle on full 7-board
        weak_slots: set[int] = set()
        if n_board > 0:
            ranked = sorted(
                range(n_board),
                key=lambda i: (
                    obs.board[i].is_golden,
                    obs.board[i].venomous,
                    obs.board[i].attack + obs.board[i].health + 4 * obs.board[i].tavern_tier,
                ),
            )
            weak_slots = set(ranked[:2])

        for act in legal:
            k = act.kind
            if k in ("end_turn", "refresh", "toggle_freeze", "dark_discovery", "pass", "reposition"):
                continue
            if k == "play":
                hi, pos = act.args
                if hi is None or hi not in canon_hand or hi >= len(obs.hand):
                    continue
                hc = obs.hand[hi]
                if hc.is_spell or hc.magnetic:
                    # Keep all distinct targeted slots for spells & magnetics (up to 7) so physics finds Amalgam/Duplicator!
                    out.append(act)
                else:
                    # Non-magnetic minion play: only need 1 insertion position (edge) + targeted battlecry positions if any
                    hk = self._hook_map.get(int(hc.card_id), (0.0,) * 6)
                    if hk[0] > 0.5 and n_board > 0:
                        # Might be a targeted battlecry -> keep up to 3 target slots (weakest, strongest, end)
                        out.append(act)
                    elif (hi, False) not in seen_play_pos:
                        seen_play_pos.add((hi, False))
                        out.append(act)
            elif k == "play_on_shop":
                hi, si = act.args
                if hi is not None and hi in canon_hand:
                    out.append(act)
            elif k == "sell":
                bi = act.args[0]
                if bi is None or bi >= n_board:
                    continue
                if n_board >= 7:
                    if bi in weak_slots or obs.board[bi].attack + obs.board[bi].health <= 30:
                        out.append(act)
                else:
                    # Only sell on <7 board if it directly unlocks Buy or Upgrade!
                    if bi in weak_slots and (
                        obs.gold + 1 == obs.minion_cost or obs.gold + 1 == obs.upgrade_cost
                    ):
                        out.append(act)
            elif k == "buy":
                si = act.args[0]
                if si is not None and si < len(obs.shop):
                    if len(obs.hand) < 10:
                        out.append(act)
            else:
                # upgrade, activate, hero_power, hero_power_2
                out.append(act)
        return out

    # -------------------------------------------------------------------------
    # Fast Greedy Rollout Consumer (for CRN Refresh / Discover / Trinket evaluation)
    # -------------------------------------------------------------------------
    def _auto_play_newest_card(
        self,
        cg: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        opp_max_hp: int,
    ) -> seaglass.TavernGame:
        """Generically plays the newest hand card (including full-7-board sell-swap if strictly higher value!)."""
        co = cg.observe()
        if co.discover_pending is not None or not co.hand:
            return cg
        nhi = len(co.hand) - 1
        nc = co.hand[nhi]
        if nc.unplayable:
            return cg
        # Try direct play / magnetize / spell cast (try best target if targeted)
        play_acts = [pa for pa in cg.legal_actions() if pa.kind == "play" and pa.args[0] == nhi]
        if play_acts:
            if len(play_acts) == 1:
                cg.step(play_acts[0])
                return cg
            sub_cands = []
            for pa in play_acts[:7]:
                sg = cg.copy()
                sg.step(pa)
                sub_cands.append(sg)
            sc = self._fast_score_obs_batch([x.observe() for x in sub_cands], ctx, opp_max_hp, end_of_turn=False)
            return sub_cands[int(np.argmax(sc))]
        # Full 7-board: test selling weakest board slot + playing newest minion!
        if len(co.board) >= 7 and not nc.is_spell:
            wi = min(
                range(len(co.board)),
                key=lambda i: (co.board[i].is_golden, co.board[i].venomous, co.board[i].attack + co.board[i].health + 3 * co.board[i].tavern_tier),
            )
            s_act = seaglass.TavernAction.sell(wi)
            if cg.is_legal(s_act):
                sg = cg.copy()
                sg.step(s_act)
                p_act = seaglass.TavernAction.play(nhi, len(sg.observe().board))
                if sg.is_legal(p_act):
                    sg.step(p_act)
                    sc_pair = self._fast_score_obs_batch([co, sg.observe()], ctx, opp_max_hp, end_of_turn=False)
                    if sc_pair[1] > sc_pair[0] - 0.5:
                        return sg
        return cg

    def _fast_rollout(
        self,
        start_game: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        opp_max_hp: int,
        max_steps: int = 6,
        seed: int = 777,
    ) -> seaglass.TavernGame:
        sim = start_game.copy()
        sim.reseed(seed | 1)
        rolls_left = 3
        for step_i in range(max_steps + 3):
            obs = sim.observe()
            if obs.discover_pending is not None or obs.trinket_options:
                d_leg = sim.legal_actions()
                if not d_leg:
                    break
                cands = []
                for a in d_leg:
                    cg = sim.copy()
                    cg.reseed((seed + step_i * 97 + 3) | 1)
                    cg.step(a)
                    cg = self._auto_play_newest_card(cg, ctx, opp_max_hp)
                    cands.append(cg)
                sc = self._fast_score_obs_batch([c.observe() for c in cands], ctx, opp_max_hp, end_of_turn=False)
                sim = cands[int(np.argmax(sc))]
                continue

            cur_sc = self._fast_score_obs_batch([obs], ctx, opp_max_hp, end_of_turn=False)[0]
            acts = self._filter_epoch_actions(obs, sim.legal_actions())
            cands = []
            for a in acts[:16]:
                cg = sim.copy()
                cg.reseed((seed + step_i * 131 + 7) | 1)
                cg.step(a)
                if a.kind == "buy":
                    cg = self._auto_play_newest_card(cg, ctx, opp_max_hp)
                cands.append(cg)

            # Also test generic full-board Sell(weakest) + Play(hand_i) inside rollout!
            if len(obs.board) >= 7 and obs.hand:
                wi = min(
                    range(len(obs.board)),
                    key=lambda i: (obs.board[i].is_golden, obs.board[i].venomous, obs.board[i].attack + obs.board[i].health),
                )
                s_act = seaglass.TavernAction.sell(wi)
                if sim.is_legal(s_act):
                    for hi, hc in enumerate(obs.hand[:3]):
                        if not hc.is_spell and not hc.unplayable and not hc.magnetic:
                            cg = sim.copy()
                            cg.step(s_act)
                            p_act = seaglass.TavernAction.play(hi, len(cg.observe().board))
                            if cg.is_legal(p_act):
                                cg.step(p_act)
                                cands.append(cg)

            if cands:
                scores = self._fast_score_obs_batch([c.observe() for c in cands], ctx, opp_max_hp, end_of_turn=False)
                bi = int(np.argmax(scores))
                if scores[bi] > cur_sc - 0.05:
                    sim = cands[bi]
                    continue
            # If nothing in shop improved value and we have gold, roll once more inside rollout!
            if rolls_left > 0 and (obs.free_refreshes > 0 or obs.gold >= obs.refresh_cost + obs.minion_cost):
                ref = seaglass.TavernAction.refresh()
                if sim.is_legal(ref):
                    sim.step(ref)
                    rolls_left -= 1
                    continue
            break
        return sim

    # -------------------------------------------------------------------------
    # Shop-Epoch Beam Search + CRN Stochastic Expectimax
    # -------------------------------------------------------------------------
    def _search_turn(
        self,
        root_game: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        is_ghost: bool,
        opp_max_hp: int,
        player: int,
        lobby_id: int,
    ) -> Tuple[Optional[seaglass.TavernAction], List[Tuple[Tuple[str, Any, Any], Tuple, bool]]]:
        root_obs = root_game.observe()
        root_fast = self._fast_score_obs_batch([root_obs], ctx, opp_max_hp, end_of_turn=False)[0]
        transp: Dict[Tuple, float] = {_canon_key(root_obs): root_fast}
        beam: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[Tuple[Tuple[str, Any, Any], Tuple, bool]]]] = [
            (root_fast, root_game, root_obs, [])
        ]
        terminals: List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, List[Tuple[Tuple[str, Any, Any], Tuple, bool]]]] = []
        roll_prefixes: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[Tuple[Tuple[str, Any, Any], Tuple, bool]]]] = []

        for _d in range(self.max_depth):
            children: List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, List[Tuple[Tuple[str, Any, Any], Tuple, bool]]]] = []
            for bf, g, obs, path in beam:
                sig = _state_sig(obs)
                terminals.append((g, obs, path + [(("end_turn", None, None), sig, False)]))
                if path and obs.discover_pending is None and not obs.trinket_options and (
                    obs.free_refreshes > 0 or obs.gold >= obs.refresh_cost + obs.minion_cost
                ):
                    roll_prefixes.append((bf, g, obs, path))

                # Also consider freezing before ending turn when low gold!
                if not obs.is_frozen and obs.shop and obs.gold < obs.minion_cost:
                    fz = seaglass.TavernAction.toggle_freeze()
                    if g.is_legal(fz):
                        gf = g.copy()
                        gf.step(fz)
                        fo = gf.observe()
                        terminals.append(
                            (gf, fo, path + [(("toggle_freeze", None, None), sig, False), (("end_turn", None, None), _state_sig(fo), False)])
                        )

                if obs.discover_pending is not None or obs.trinket_options:
                    terminals.append((g, obs, path))
                    continue

                acts = self._filter_epoch_actions(obs, g.legal_actions())
                for act in acts:
                    cg = g.copy()
                    cg.reseed(self.rng.getrandbits(63) | 1)
                    ended = cg.step(act)
                    co = cg.observe()
                    # Detect stochastic non-discover side effect cleanly
                    is_stoch = False
                    if act.kind in ("hero_power", "hero_power_2") and co.discover_pending is None:
                        is_stoch = True
                    qstep = (_action_key(act), sig, is_stoch)
                    if ended or is_stoch:
                        terminals.append((cg, co, path + [qstep]))
                    else:
                        children.append((cg, co, path + [qstep]))

                # Generic full-board Sell + Play AND Buy + Sell + Play macros (zero card hardcoding!)
                if len(obs.board) >= 7:
                    weak_i = min(
                        range(len(obs.board)),
                        key=lambda i: (obs.board[i].is_golden, obs.board[i].venomous, obs.board[i].attack + obs.board[i].health + 3 * obs.board[i].tavern_tier),
                    )
                    s_act = seaglass.TavernAction.sell(weak_i)
                    if g.is_legal(s_act):
                        # (a) Sell(weak) + Play(hand_i)
                        for hi, hc in enumerate(obs.hand):
                            if not hc.is_spell and not hc.unplayable and not hc.magnetic:
                                cg = g.copy()
                                cg.reseed(self.rng.getrandbits(63) | 1)
                                cg.step(s_act)
                                o_mid = cg.observe()
                                p_act = seaglass.TavernAction.play(hi, len(o_mid.board))
                                if cg.is_legal(p_act):
                                    cg.step(p_act)
                                    children.append(
                                        (cg, cg.observe(), path + [(_action_key(s_act), sig, False), (_action_key(p_act), _state_sig(o_mid), False)])
                                    )
                        # (b) Atomic Buy(shop_i) + Sell(weak) + Play(newest) when hand has space!
                        if len(obs.hand) < 10 and obs.gold >= obs.minion_cost:
                            for si, scard in enumerate(obs.shop):
                                if scard.is_spell or scard.magnetic:
                                    continue
                                b_act = seaglass.TavernAction.buy(si)
                                if not g.is_legal(b_act):
                                    continue
                                cg = g.copy()
                                cg.reseed(self.rng.getrandbits(63) | 1)
                                cg.step(b_act)
                                o_b = cg.observe()
                                if o_b.discover_pending is not None or not o_b.hand or len(o_b.board) < 7:
                                    continue
                                nhi = len(o_b.hand) - 1
                                if o_b.hand[nhi].unplayable or o_b.hand[nhi].is_spell:
                                    continue
                                if cg.is_legal(s_act):
                                    cg.step(s_act)
                                    o_s = cg.observe()
                                    p_act = seaglass.TavernAction.play(nhi, len(o_s.board))
                                    if cg.is_legal(p_act):
                                        cg.step(p_act)
                                        children.append(
                                            (
                                                cg,
                                                cg.observe(),
                                                path
                                                + [
                                                    (_action_key(b_act), sig, False),
                                                    (_action_key(s_act), _state_sig(o_b), False),
                                                    (_action_key(p_act), _state_sig(o_s), False),
                                                ],
                                            )
                                        )

            if not children:
                break
            dedup = []
            seen_l: set[Tuple] = set()
            for cg, co, cp in children:
                ck = _canon_key(co)
                if ck in seen_l:
                    continue
                seen_l.add(ck)
                dedup.append((ck, cg, co, cp))
            c_scores = self._fast_score_obs_batch([x[2] for x in dedup], ctx, opp_max_hp, end_of_turn=False)
            next_beam = []
            for (ck, cg, co, cp), cs in zip(dedup, c_scores):
                prev = transp.get(ck)
                if prev is not None and cs <= prev + 1e-6:
                    continue
                transp[ck] = cs
                next_beam.append((cs, cg, co, cp))
            if not next_beam:
                break
            next_beam.sort(key=lambda x: x[0], reverse=True)
            beam = next_beam[: self.beam_width]

        # Dedup & rank terminals by fast evaluator, then physics-evaluate top-K!
        t_fast = self._fast_score_obs_batch([t[1] for t in terminals], ctx, opp_max_hp, end_of_turn=True)
        seen_t: set[Tuple] = set()
        top_terms: List[Tuple[seaglass.TavernGame, List[Tuple[Tuple[str, Any, Any], Tuple, bool]]]] = []
        for _, (tg, to, tp) in sorted(zip(t_fast, terminals), key=lambda x: x[0], reverse=True):
            tk = _canon_key(to)
            if tk in seen_t:
                continue
            seen_t.add(tk)
            top_terms.append((tg, tp))
            if len(top_terms) >= self.top_k_physics:
                break

        eval_games = [
            self._fast_rollout(tg, ctx, opp_max_hp, max_steps=3, seed=self.rng.getrandbits(31))
            if tg.observe().discover_pending is not None
            else tg
            for tg, _ in top_terms
        ]
        term_vals, term_pvecs = self._evaluate_games_batch(
            eval_games, ctx, opp_board, opp_tier, is_ghost, opp_max_hp,
            combats=self.combats_eval, seed_base=root_obs.turn * 1009 + 17,
        )

        # TRUE 2-Turn CRN Expectimax Tiebreaker ONLY on first epoch of turn when forks differ in Tavern Tier!
        ref_key = (lobby_id, player, root_obs.turn)
        refs_so_far = self._ref_count.get(ref_key, 0)
        alive_units = ctx.alive_units if ctx is not None else 8
        is_finals = alive_units <= 2 and not is_ghost
        if (
            self.two_turn_lookahead
            and refs_so_far == 0
            and not is_finals
            and 4 <= root_obs.turn <= 11
            and len(eval_games) >= 2
        ):
            ranked_i = sorted(range(len(eval_games)), key=lambda idx: term_vals[idx], reverse=True)
            fork_idxs: List[int] = [ranked_i[0]]
            seen_tiers = {eval_games[ranked_i[0]].observe().tavern_tier}
            for idx in ranked_i[1:]:
                ti = eval_games[idx].observe().tavern_tier
                if ti not in seen_tiers:
                    fork_idxs.append(idx)
                    seen_tiers.add(ti)
                if len(fork_idxs) >= 2:
                    break
            if len(fork_idxs) >= 2:
                crn2 = [self.rng.getrandbits(63) | 1, self.rng.getrandbits(63) | 1]
                t1_all: List[seaglass.TavernGame] = []
                for idx in fork_idxs:
                    for s2 in crn2:
                        sg = eval_games[idx].copy()
                        sg.reseed(s2)
                        self._fast_generic_order(sg)
                        if sg.is_legal(seaglass.TavernAction.end_turn()):
                            sg.end_turn()
                        if opp_board:
                            sg.resolve_combat_against(opp_board, opponent_tier=opp_tier, seed=(s2 ^ 0x5678) & 0x7FFFFFFF)
                        if sg.observe().health > 0:
                            sg.start_turn()
                            sg = self._fast_rollout(sg, ctx, opp_max_hp, max_steps=5, seed=(s2 ^ 0x9ABC) & 0x7FFFFFFF)
                        t1_all.append(sg)
                v1_all, _ = self._evaluate_games_batch(
                    t1_all, ctx, opp_board, opp_tier, False, opp_max_hp,
                    combats=max(4, self.combats_eval // 2), seed_base=root_obs.turn * 3037 + 19,
                )
                for k_f, idx in enumerate(fork_idxs):
                    m1 = float(np.mean(v1_all[k_f * 2 : (k_f + 1) * 2]))
                    term_vals[idx] = 0.52 * term_vals[idx] + 0.48 * m1

        best_i = int(np.argmax(term_vals))
        best_val = term_vals[best_i]
        best_path = top_terms[best_i][1]
        best_imm: Optional[seaglass.TavernAction] = None
        self.last_physics_vec = term_pvecs[best_i]

        # CRN-Paired Expectimax for Stochastic Actions (`DarkDiscovery`, `Refresh`, top `Action -> Refresh` prefixes!)
        crn_seeds = [self.rng.getrandbits(63) | 1 for _ in range(self.refresh_samples)]

        def _eval_stoch(base_g: seaglass.TavernGame, act: seaglass.TavernAction) -> float:
            sims = []
            for s in crn_seeds:
                sg = base_g.copy()
                sg.reseed(s)
                sg.step(act)
                sims.append(self._fast_rollout(sg, ctx, opp_max_hp, max_steps=5, seed=(s ^ 0xA5A5) & 0x7FFFFFFF))
            vals, _ = self._evaluate_games_batch(
                sims, ctx, opp_board, opp_tier, is_ghost, opp_max_hp,
                combats=max(4, self.combats_eval // 2), seed_base=root_obs.turn * 2027 + 5,
            )
            return float(np.mean(vals))

        # 1. `DarkDiscovery`
        dd_act = seaglass.TavernAction.dark_discovery()
        if root_game.is_legal(dd_act):
            dd_v = _eval_stoch(root_game, dd_act)
            if dd_v > best_val + 0.3:
                best_val = dd_v
                best_imm = dd_act

        # 2. Root `Refresh`
        ref_act = seaglass.TavernAction.refresh()
        if (
            refs_so_far < 10
            and root_game.is_legal(ref_act)
            and (root_obs.free_refreshes > 0 or root_obs.gold >= root_obs.refresh_cost + root_obs.minion_cost)
        ):
            rv = _eval_stoch(root_game, ref_act)
            if rv > best_val + 0.25:
                best_val = rv
                best_imm = ref_act

        # 3. `Sell(weak) -> Refresh` when 1 gold shy of rolling + buying!
        if (
            refs_so_far < 10
            and root_obs.free_refreshes == 0
            and root_obs.gold < root_obs.refresh_cost + root_obs.minion_cost
            and root_obs.gold + 1 >= root_obs.refresh_cost + root_obs.minion_cost
            and len(root_obs.board) >= 6
        ):
            wi = min(
                range(len(root_obs.board)),
                key=lambda i: (root_obs.board[i].is_golden, root_obs.board[i].venomous, root_obs.board[i].attack + root_obs.board[i].health + 3 * root_obs.board[i].tavern_tier),
            )
            sa = seaglass.TavernAction.sell(wi)
            if root_game.is_legal(sa):
                sg_s = root_game.copy()
                sg_s.step(sa)
                if sg_s.is_legal(ref_act):
                    sv = _eval_stoch(sg_s, ref_act)
                    if sv > best_val + 0.5:
                        best_val = sv
                        best_imm = None
                        best_path = [
                            (_action_key(sa), _state_sig(root_obs), False),
                            (("refresh", None, None), _state_sig(sg_s.observe()), True),
                        ]

        # 3. Top-2 `Action -> Refresh` prefixes (e.g., Buy engine -> Refresh, Upgrade -> Refresh!)
        if refs_so_far < 10 and roll_prefixes:
            roll_prefixes.sort(key=lambda x: x[0], reverse=True)
            seen_rp: set[Tuple] = set()
            tested = 0
            for _, pg, po, pp in roll_prefixes:
                ck = _canon_key(po)
                if ck in seen_rp or not pg.is_legal(ref_act):
                    continue
                seen_rp.add(ck)
                pv = _eval_stoch(pg, ref_act)
                if pv > best_val + 0.25:
                    best_val = pv
                    best_imm = None
                    best_path = pp + [(("refresh", None, None), _state_sig(po), True)]
                tested += 1
                if tested >= 2:
                    break

        return best_imm, best_path

    # -------------------------------------------------------------------------
    # Prompts: Hero, Trinket (2-Turn CRN Physics!), Discover (CRN Continuation!)
    # -------------------------------------------------------------------------
    def _choose_hero(
        self,
        base_g: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        ctx: Optional[seaglass.LobbyContext],
    ) -> seaglass.TavernAction:
        obs_list = []
        acts = []
        for a in legal:
            if a.kind != "choose_hero":
                continue
            sg = base_g.copy()
            if sg.is_legal(a):
                sg.step(a)
                obs_list.append(sg.observe())
                acts.append(a)
        if not acts:
            return legal[0]
        if self.has_net and self.predictor is not None:
            utils, _ = self.predictor.predict_batch(obs_list, ctx=ctx, physics_list=None, end_of_turn=False)
            if self.explore_noise > 0.0:
                utils = utils + np.array([self.rng.gauss(0.0, self.explore_noise) for _ in utils])
            return acts[int(np.argmax(utils))]
        # Gen-0 pure fallback: highest starting EHP (zero hardcoded hero table!)
        return acts[int(np.argmax([o.health + o.armor for o in obs_list]))]

    def _choose_trinket(
        self,
        base_g: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        is_ghost: bool,
        opp_max_hp: int,
    ) -> seaglass.TavernAction:
        # 2-Turn Paired-Seed CRN Physics + ZeroValueNet Rollout (zero hardcoded trinket table!)
        crn = [self.rng.getrandbits(63) | 1, self.rng.getrandbits(63) | 1]
        best_a = legal[0]
        best_v = -1e18
        obs0 = base_g.observe()
        for a in legal:
            if a.kind != "choose_trinket":
                continue
            idx = a.args[0]
            impl_pen = 0.0
            if idx is not None and idx < len(obs0.trinket_options):
                if not self.vocab.trinket_impl.get(int(obs0.trinket_options[idx]), True):
                    impl_pen = -15.0
            t0_sims = []
            t1_sims = []
            for s in crn:
                sg = base_g.copy()
                sg.reseed(s)
                sg.step(a)
                sg0 = self._fast_rollout(sg, ctx, opp_max_hp, max_steps=4, seed=(s ^ 0x1234) & 0x7FFFFFFF)
                t0_sims.append(sg0)
                sg1 = sg0.copy()
                self._fast_generic_order(sg1)
                if sg1.is_legal(seaglass.TavernAction.end_turn()):
                    sg1.end_turn()
                if opp_board:
                    sg1.resolve_combat_against(opp_board, opponent_tier=opp_tier, seed=(s ^ 0x5678) & 0x7FFFFFFF)
                if sg1.observe().health > 0:
                    sg1.start_turn()
                    sg1 = self._fast_rollout(sg1, ctx, opp_max_hp, max_steps=4, seed=(s ^ 0x9ABC) & 0x7FFFFFFF)
                t1_sims.append(sg1)
            v0, _ = self._evaluate_games_batch(t0_sims, ctx, opp_board, opp_tier, is_ghost, opp_max_hp, combats=6, seed_base=obs0.turn * 331)
            v1, _ = self._evaluate_games_batch(t1_sims, ctx, opp_board, opp_tier, is_ghost, opp_max_hp, combats=6, seed_base=obs0.turn * 661)
            tot = 0.55 * float(np.mean(v0)) + 0.45 * float(np.mean(v1)) + impl_pen
            if self.explore_noise > 0.0:
                tot += self.rng.gauss(0.0, self.explore_noise)
            if tot > best_v:
                best_v = tot
                best_a = a
        return best_a

    def _choose_discover(
        self,
        base_g: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        is_ghost: bool,
        opp_max_hp: int,
    ) -> seaglass.TavernAction:
        crn = self.rng.getrandbits(63) | 1
        sims = []
        acts = []
        for a in legal:
            if a.kind != "choose_discover":
                continue
            sg = base_g.copy()
            sg.reseed(crn)
            sg.step(a)
            sims.append(self._fast_rollout(sg, ctx, opp_max_hp, max_steps=3, seed=(crn ^ 0x7777) & 0x7FFFFFFF))
            acts.append(a)
        if not acts:
            return legal[0]
        vals, _ = self._evaluate_games_batch(sims, ctx, opp_board, opp_tier, is_ghost, opp_max_hp, combats=6, seed_base=base_g.observe().turn * 997)
        return acts[int(np.argmax(vals))]

    # -------------------------------------------------------------------------
    # Pure-Physics End-of-Turn Combat Positioning (Zero Card Hardcoding!)
    # -------------------------------------------------------------------------
    def _best_position_actions(
        self,
        base_g: seaglass.TavernGame,
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        ctx: Optional[seaglass.LobbyContext],
    ) -> List[seaglass.TavernAction]:
        obs = base_g.observe()
        n = len(obs.board)
        if n <= 1 or not opp_board:
            return []
        alive_units = ctx.alive_units if ctx is not None else 8
        combats = 20 if alive_units <= 2 else (12 if alive_units <= 4 else 8)

        def _perm_acts(perm: Sequence[int]) -> List[seaglass.TavernAction]:
            cur = list(range(n))
            out: List[seaglass.TavernAction] = []
            for dst, orig in enumerate(perm):
                src = cur.index(orig)
                if src != dst:
                    out.append(seaglass.TavernAction.reposition(src, dst))
                    cur.insert(dst, cur.pop(src))
            return out

        def _score_perm(perm: Sequence[int]) -> float:
            sg = base_g.copy()
            for a in _perm_acts(perm):
                if sg.is_legal(a):
                    sg.step(a)
            if sg.is_legal(seaglass.TavernAction.end_turn()):
                sg.end_turn()
            dist = sg.equity_against(
                opp_board,
                opponent_tier=opp_tier,
                n=combats,
                base_seed=obs.turn * 7919 + 11,
            )
            return 100.0 * dist.a_win_rate + 35.0 * dist.draw_rate + 0.9 * dist.damage_mean

        # 2-Pass Pure-Physics Hill Climbing (pairwise swaps + move-to-edges; zero card names!)
        best_p: Tuple[int, ...] = tuple(range(n))
        best_ev = _score_perm(best_p)
        tested: set[Tuple[int, ...]] = {best_p}
        passes = 2 if n >= 4 else 1
        for _pass in range(passes):
            improved = False
            nbrs: List[Tuple[int, ...]] = []
            base_l = list(best_p)
            for i in range(n):
                for j in range(i + 1, n):
                    p = list(base_l)
                    p[i], p[j] = p[j], p[i]
                    tp = tuple(p)
                    if tp not in tested:
                        tested.add(tp)
                        nbrs.append(tp)
            # Also try shifting each unit to slot 0 (leftmost) or slot n-1 (rightmost)
            for i in range(n):
                for edge in (0, n - 1):
                    if i != edge:
                        p = list(base_l)
                        v = p.pop(i)
                        p.insert(edge, v)
                        tp = tuple(p)
                        if tp not in tested:
                            tested.add(tp)
                            nbrs.append(tp)
            for perm in nbrs:
                ev = _score_perm(perm)
                if ev > best_ev + 1e-4:
                    best_ev = ev
                    best_p = perm
                    improved = True
            if not improved:
                break
        return _perm_acts(best_p)

    # -------------------------------------------------------------------------
    # Main Lobby Entrypoint
    # -------------------------------------------------------------------------
    def act_in_lobby(
        self,
        lobby: seaglass.LobbyGame,
        player: int,
    ) -> seaglass.TavernAction:
        obs = lobby.observe(player)
        ctx = lobby.context(player)
        legal = lobby.legal_actions(player)
        self.last_note = None
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            return legal[0]

        base_g = lobby.belief_tavern(player, seed=self.rng.getrandbits(63) | 1, extra_units=[])
        lobby_id = id(lobby)

        if legal[0].kind == "choose_hero":
            return self._choose_hero(base_g, legal, ctx)

        opp_board, opp_tier, is_ghost, opp_max_hp = self._opponent_board_and_tier(obs, ctx)

        if legal[0].kind == "choose_trinket":
            self._queued.pop(player, None)
            return self._choose_trinket(base_g, legal, ctx, opp_board, opp_tier, is_ghost, opp_max_hp)

        if legal[0].kind == "choose_discover":
            self._queued.pop(player, None)
            return self._choose_discover(base_g, legal, ctx, opp_board, opp_tier, is_ghost, opp_max_hp)

        def _match(akey: Tuple[str, Any, Any]) -> Optional[seaglass.TavernAction]:
            for a in legal:
                if _action_key(a) == akey:
                    return a
            return None

        # Drain queued shop-epoch plan
        q = self._queued.get(player)
        if q:
            ak, exp_sig, stop_after = q.pop(0)
            if _state_sig(obs) == exp_sig:
                if ak[0] == "end_turn":
                    self._queued.pop(player, None)
                    return self._finish_or_position(base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, ctx)
                m = _match(ak)
                if m is not None:
                    if m.kind == "refresh":
                        rk = (lobby_id, player, obs.turn)
                        self._ref_count[rk] = self._ref_count.get(rk, 0) + 1
                    if stop_after or not q:
                        self._queued.pop(player, None)
                    return m
            self._queued.pop(player, None)

        imm, path = self._search_turn(
            base_g, ctx, opp_board, opp_tier, is_ghost, opp_max_hp, player, lobby_id
        )
        if imm is not None:
            self._queued.pop(player, None)
            if imm.kind == "refresh":
                rk = (lobby_id, player, obs.turn)
                self._ref_count[rk] = self._ref_count.get(rk, 0) + 1
            m = _match(_action_key(imm))
            if m is not None:
                return m

        if not path or path[0][0][0] == "end_turn":
            return self._finish_or_position(base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, ctx)

        first_ak, _, first_stop = path[0]
        if len(path) > 1 and not first_stop:
            self._queued[player] = path[1:]
        else:
            self._queued.pop(player, None)
        m = _match(first_ak)
        if m is not None:
            if m.kind == "refresh":
                rk = (lobby_id, player, obs.turn)
                self._ref_count[rk] = self._ref_count.get(rk, 0) + 1
            return m
        self._queued.pop(player, None)
        return self._finish_or_position(base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, ctx)

    def _finish_or_position(
        self,
        base_g: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        player: int,
        turn: int,
        lobby_id: int,
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        ctx: Optional[seaglass.LobbyContext],
    ) -> seaglass.TavernAction:
        ptag = (lobby_id, turn)
        if self._pos_done.get(player) != ptag and len(base_g.observe().board) >= 2:
            self._pos_done[player] = ptag
            repo_acts = self._best_position_actions(base_g, opp_board, opp_tier, ctx)
            if repo_acts:
                sim_p = base_g.copy()
                steps = []
                for a in repo_acts:
                    steps.append((_action_key(a), _state_sig(sim_p.observe()), False))
                    if sim_p.is_legal(a):
                        sim_p.step(a)
                steps.append((("end_turn", None, None), _state_sig(sim_p.observe()), False))
                for la in legal:
                    if _action_key(la) == steps[0][0]:
                        self._queued[player] = steps[1:]
                        return la
        for la in legal:
            if la.kind == "end_turn":
                return la
        return legal[0]
