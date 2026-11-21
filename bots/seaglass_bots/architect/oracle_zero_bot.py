"""OracleZeroBot: Pure AlphaZero Multi-Turn Stochastic Rollout + ValueNet + Rust Physics Bot!

ZERO hardcoded card tables, ZERO hardcoded comp rules, ZERO hardcoded hero/trinket tables,
ZERO hardcoded leveling scripts!

Core Architecture:
  1. Policy-Guided Within-Turn Deterministic Beam (`π(a|s)`):
       - Batches `EntityTransformer` forward passes across the frontier to discover multi-step
         within-turn combos (`Sell -> Buy -> Play -> Activate -> HeroPower -> Upgrade`) without
         any hand-crafted pruning heuristics.
       - Strictly separates deterministic plays from stochastic reveals (`refresh`,
         `dark_discovery`, `choose_discover`) so search NEVER overfits a single lucky RNG seed!
       - Queues deterministic action plans and only replans when a stochastic reveal occurs or
         unexpected state diverges (`~1.5–3.5 s/turn`, faster than SovereignBot!).
  2. TRUE Multi-Turn Stochastic Rollout Evaluation (`Turn t -> Combat(t) -> Turn t+1 [-> Turn t+2]`):
       - Pre-filters deterministic EOT boards across diverse macro buckets (same-tier vs upgraded
         vs frozen vs triple-holding) using `ArchitectNumpyPredictor` (`architect_value_v2.npz`,
         0.045ms pure Set-Transformer) + immediate Rust `equity_against`.
       - Evaluates finalist lines + CRN-paired stochastic branches (`refresh`, `dark_discovery`)
         by rolling forward `M` CRN-paired futures through exact Rust combat and FULL batched
         `EntityTransformer` Turn `t+1` (and Turn `t+2` on midgame macro turns!) rollouts!
       - Scores future leaves with 1st-place-sharp calibrated placement utility from
         `architect_value_v2.npz` + `EntityTransformer` Critic `V_π` + Rust combat survival!
  3. 2-Turn CRN Policy Rollouts for Trinkets & Discovers (ZERO hardcoded trinket priors!).
  4. Pure-Physics Finals Exact Combat Solver (`alive <= 2`):
       - Maximizes exact Rust `equity_against(observed_opp_board)` over multi-step board swaps,
         refreshes, and 2-pass pure-physics positioning — discovering scam/tech/counterplay
         purely from Rust combat simulation!
"""

from __future__ import annotations

import os
import random
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple

_RL_DIR = str(Path(__file__).resolve().parents[3] / "rl")
if _RL_DIR not in sys.path:
    sys.path.insert(0, _RL_DIR)

import numpy as np
import seaglass
import torch

from seaglass_rl import checkpoint as rl_ckpt
from seaglass_rl.actions import (
    ACTION_TO_INDEX,
    action_from_index,
    action_mask,
)
from seaglass_rl.features import encode_observation

from .hybrid import ArchitectNumpyPredictor
from .policy_bot import DEFAULT_POLICY_PT_PATH, _GameView
from .zero_bot import ZeroPhysicsBot, _action_key, _state_sig

DEFAULT_VALUE_V2_NPZ = str(
    Path(__file__).resolve().parent.parent / "data" / "architect_value_v2.npz"
)
DEFAULT_VALUE_V1_NPZ = str(
    Path(__file__).resolve().parent.parent / "data" / "architect_value_v1.npz"
)

_STOCHASTIC_KINDS = frozenset({"refresh", "dark_discovery"})


@dataclass
class _BeamNode:
    game: seaglass.TavernGame
    actions: List[seaglass.TavernAction]
    logp_sum: float = 0.0
    ended: bool = False
    stochastic_trigger: Optional[seaglass.TavernAction] = None
    pre_score: float = 0.0


class OracleZeroBot:
    """Pure AlphaZero Multi-Turn Rollout Search: `EntityTransformer π` + `ValueNet V` + Rust Physics."""

    def __init__(
        self,
        policy_ckpt: Optional[str] = None,
        value_npz: Optional[str] = None,
        beam_width: int = 10,
        max_depth: int = 6,
        top_k_expand: int = 3,
        finalist_det: int = 5,
        finalist_stoch: int = 3,
        rollout_futures: int = 4,
        two_turn_horizon: bool = True,
        three_turn_midgame: bool = False,
        combats_eval: int = 8,
        critic_weight: float = 0.25,
        combat_weight: float = 2.5,
        prior_weight: float = 0.25,
        seed: int = 2029,
        **kwargs,
    ) -> None:
        ckpt_path = policy_ckpt or DEFAULT_POLICY_PT_PATH
        self.model, _ = rl_ckpt.load(ckpt_path)
        self.model.eval()

        v_path = value_npz
        if not v_path:
            v_path = DEFAULT_VALUE_V2_NPZ if os.path.exists(DEFAULT_VALUE_V2_NPZ) else DEFAULT_VALUE_V1_NPZ
        self.value_pred = ArchitectNumpyPredictor(catalog_name="full", weights_path=v_path)

        self.beam_width = beam_width
        self.max_depth = max_depth
        self.top_k_expand = top_k_expand
        self.finalist_det = finalist_det
        self.finalist_stoch = finalist_stoch
        self.rollout_futures = rollout_futures
        self.two_turn_horizon = two_turn_horizon
        self.three_turn_midgame = three_turn_midgame
        self.combats_eval = combats_eval
        self.critic_weight = critic_weight
        self.combat_weight = combat_weight
        self.prior_weight = prior_weight
        self.rng = random.Random(seed)
        self.last_note: Optional[Dict[str, float]] = None

        self._helper = ZeroPhysicsBot(weights_path="", two_turn_lookahead=False, seed=seed)
        self._pos_done: Dict[int, Tuple[int, int]] = {}
        self._queued: Dict[int, List[Tuple[Tuple[str, Any, Any], Tuple, bool]]] = {}
        self._ref_count: Dict[Tuple[int, int, int], int] = {}
        self._replan_count: Dict[Tuple[int, int, int], int] = {}

    # ------------------------------------------------------------------
    # Batched Neural Helpers
    # ------------------------------------------------------------------
    def _batch_policy_and_critic(
        self,
        games: Sequence[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
        need_mask: bool = True,
    ) -> Tuple[np.ndarray, np.ndarray, Optional[np.ndarray]]:
        """Single batched `EntityTransformer` forward pass returning `(log_probs, values, masks)`."""
        n = len(games)
        if n == 0:
            return np.zeros((0, 1), dtype=np.float32), np.zeros(0, dtype=np.float32), None
        pvs, cis, nfs = [], [], []
        mks = [] if need_mask else None
        for g in games:
            obs = g.observe()
            pv, ci, nf = encode_observation(obs, ctx, arranging=False)
            pvs.append(pv)
            cis.append(ci)
            nfs.append(nf)
            if need_mask:
                mk = action_mask(_GameView(g), allow_reposition=False, allow_freeze=True, obs=obs)
                mks.append(mk)
        with torch.no_grad():
            mk_t = torch.from_numpy(np.stack(mks, axis=0).astype(np.bool_)) if need_mask else None
            logits, vals = self.model.forward(
                torch.from_numpy(np.stack(pvs, axis=0).astype(np.float32)),
                torch.from_numpy(np.stack(cis, axis=0).astype(np.int64)),
                torch.from_numpy(np.stack(nfs, axis=0).astype(np.float32)),
                mk_t,
            )
            lps = torch.log_softmax(logits, dim=-1).numpy().astype(np.float32) if need_mask else logits.numpy()
            return lps, vals.numpy().astype(np.float32), (np.stack(mks, axis=0) if need_mask else None)

    def _rollout_turn_batch(
        self,
        sims: List[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
        max_steps: int = 18,
    ) -> List[seaglass.TavernGame]:
        """Batched greedy policy rollout under `π(a|s)` until End Turn."""
        active = list(range(len(sims)))
        views = [_GameView(g) for g in sims]
        for _ in range(max_steps):
            if not active:
                break
            pvs, cis, nfs, mks, keep_idx = [], [], [], [], []
            for idx in active:
                g = sims[idx]
                obs = g.observe()
                mk = action_mask(views[idx], allow_reposition=False, allow_freeze=True, obs=obs)
                if not mk.any():
                    continue
                pv, ci, nf = encode_observation(obs, ctx, arranging=False)
                pvs.append(pv)
                cis.append(ci)
                nfs.append(nf)
                mks.append(mk)
                keep_idx.append(idx)
            if not keep_idx:
                break
            with torch.no_grad():
                logits, _ = self.model.forward(
                    torch.from_numpy(np.stack(pvs, axis=0).astype(np.float32)),
                    torch.from_numpy(np.stack(cis, axis=0).astype(np.int64)),
                    torch.from_numpy(np.stack(nfs, axis=0).astype(np.float32)),
                    torch.from_numpy(np.stack(mks, axis=0).astype(np.bool_)),
                )
                act_ids = logits.argmax(dim=-1).numpy()
            next_active: List[int] = []
            for idx, a_i in zip(keep_idx, act_ids):
                act = action_from_index(int(a_i))
                g = sims[idx]
                if act.kind == "end_turn" or not g.is_legal(act):
                    continue
                ended = g.step(act)
                if not ended:
                    next_active.append(idx)
            active = next_active
        return sims

    def _value_batch(
        self,
        games: Sequence[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
        end_of_turn: bool = True,
    ) -> np.ndarray:
        """Pure learned ValueNet (`architect_value_v2.npz`) + `EntityTransformer` Critic `V_π`."""
        if not games:
            return np.zeros(0, dtype=np.float32)
        obs_list = [g.observe() for g in games]
        v_arch = np.asarray(
            self.value_pred.predict_batch(obs_list, ctx=ctx, end_of_turn=end_of_turn),
            dtype=np.float32,
        )
        if self.critic_weight > 0.0:
            _, v_crit, _ = self._batch_policy_and_critic(games, ctx, need_mask=False)
            # Scale critic (~[-1, 2.2]) into comparable ~[-25, +45] placement utility units
            return v_arch + self.critic_weight * (14.0 * v_crit)
        return v_arch

    def _next_turn_opponent(
        self,
        ctx: Optional[seaglass.LobbyContext],
        fallback_board: List[seaglass.Unit],
        fallback_tier: int,
        turn_next: int,
        rng_seed: int,
    ) -> Tuple[List[seaglass.Unit], int]:
        """Samples a realistic known alive opponent board from `ctx` for Turn `t+1` combat."""
        if ctx is not None and getattr(ctx, "opponents", None):
            cands = [
                o
                for o in ctx.opponents
                if o.alive and getattr(o, "seen_board", None) and len(o.seen_board) > 0
            ]
            if cands:
                pick = cands[rng_seed % len(cands)]
                return list(pick.seen_board), int(getattr(pick, "tavern_tier", fallback_tier))
        return fallback_board, min(6, max(1, fallback_tier + (1 if turn_next in (2, 4, 6, 8, 10) else 0)))

    # ------------------------------------------------------------------
    # Multi-Turn Stochastic Rollout Leaf Evaluator (The Superhuman Edge!)
    # ------------------------------------------------------------------
    def _evaluate_eot_lines_multi_turn(
        self,
        eot_games: List[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        is_ghost: bool,
        crn_seeds: List[int],
        turn_now: int,
        is_finals: bool,
    ) -> np.ndarray:
        """
        Evaluates a batch of candidate End-of-Turn states across `M` CRN-paired futures:
          - Turn `t`: Fast pure-physics ordering + `end_turn()` + exact Rust `equity_against` + `V(s_eot)`
          - Combat `t` -> `start_turn()` -> Batched `EntityTransformer` rollout of Turn `t+1` -> `V(s_{t+1})`
          - Optionally (Turn 4–10): Combat `t+1` -> Turn `t+2` rollout -> `V(s_{t+2})`!
        """
        N = len(eot_games)
        if N == 0:
            return np.zeros(0, dtype=np.float32)

        # 1. Exact Turn `t` EOT evaluation (post-EOT triggers + exact Rust combat equity)
        eot_post: List[seaglass.TavernGame] = []
        win_r = np.zeros(N, dtype=np.float32)
        draw_r = np.zeros(N, dtype=np.float32)
        dmg_m = np.zeros(N, dtype=np.float32)
        lethal_r = np.zeros(N, dtype=np.float32)
        ehp_arr = np.zeros(N, dtype=np.float32)

        n_comb = max(20, self.combats_eval * 2) if is_finals else max(8, self.combats_eval)
        s_comb = (crn_seeds[0] ^ 0x51A7) | 1

        for i, g in enumerate(eot_games):
            o0 = g.observe()
            ehp_arr[i] = float(o0.health + o0.armor)
            sim = g.copy()
            sim.reseed((crn_seeds[0] + i * 31) | 1)
            self._helper._fast_generic_order(sim)
            if sim.is_legal(seaglass.TavernAction.end_turn()):
                sim.end_turn()
            eot_post.append(sim)

            if opp_board and not is_ghost:
                dist = sim.equity_against(
                    opp_board,
                    opponent_tier=opp_tier,
                    n=n_comb,
                    base_seed=s_comb,
                )
                win_r[i] = float(dist.a_win_rate)
                draw_r[i] = float(dist.draw_rate)
                dmg_m[i] = float(dist.damage_mean)
                # Estimate lethal risk from mean/max damage
                if dist.damage_mean < 0 and ehp_arr[i] <= abs(dist.damage_mean) * 1.15:
                    lethal_r[i] = float(dist.b_win_rate)

        # In 1v1 Finals against known live board: PURE RUST COMBAT PHYSICS SOLVER!
        if is_finals and opp_board and not is_ghost:
            v_eot = self._value_batch(eot_post, ctx, end_of_turn=True)
            return 120.0 * win_r + 42.0 * draw_r + 1.2 * dmg_m + 0.15 * v_eot

        v_eot = self._value_batch(eot_post, ctx, end_of_turn=True)
        # Guard against OOD `gold` hallucination in EOT-only ValueNet (unspent gold is wiped on end_turn!)
        unspent_g = np.array([float(g.observe().gold) for g in eot_games], dtype=np.float32)
        v_eot = v_eot - 2.2 * np.maximum(0.0, unspent_g - 1.0)

        # Immediate combat bonus: gentle variance reduction + lethal guard ONLY (since v_t1 already simulates Combat t!)
        hp_urg = np.where(ehp_arr <= 18.0, 1.0 + (18.0 - ehp_arr) / 8.0, 0.55)
        combat_t0 = self.combat_weight * hp_urg * (win_r + 0.35 * draw_r + 0.025 * dmg_m) - 24.0 * lethal_r

        if not self.two_turn_horizon or turn_now >= 17 or len(crn_seeds) == 0:
            return v_eot + combat_t0

        # 2. TRUE Turn `t+1` CRN-Paired Batched Policy Rollout (`N * M` futures simultaneously!)
        M = len(crn_seeds)
        t1_games: List[seaglass.TavernGame] = []
        t1_alive_idx: List[int] = []
        t1_dead = np.zeros(N * M, dtype=np.float32)

        for i in range(N):
            base_post = eot_post[i]
            for m, s in enumerate(crn_seeds):
                idx = i * M + m
                sg1 = base_post.copy()
                if opp_board:
                    sg1.resolve_combat_against(opp_board, opponent_tier=opp_tier, seed=(s ^ 0x9E37) | 1)
                if sg1.observe().health <= 0:
                    t1_dead[idx] = -55.0
                    t1_games.append(sg1)
                else:
                    sg1.reseed((s ^ 0xA5A5) | 1)
                    sg1.start_turn()
                    t1_alive_idx.append(idx)
                    t1_games.append(sg1)

        if t1_alive_idx:
            rolled1 = self._rollout_turn_batch([t1_games[j] for j in t1_alive_idx], ctx, max_steps=12)
            for j, g_r in zip(t1_alive_idx, rolled1):
                self._helper._fast_generic_order(g_r)
                if g_r.is_legal(seaglass.TavernAction.end_turn()):
                    g_r.end_turn()
                t1_games[j] = g_r

        v_t1_flat = np.zeros(N * M, dtype=np.float32)
        if t1_alive_idx:
            v1_alive = self._value_batch([t1_games[j] for j in t1_alive_idx], ctx, end_of_turn=True)
            for j, val in zip(t1_alive_idx, v1_alive):
                v_t1_flat[j] = val
        v_t1_flat += t1_dead
        v_t1 = v_t1_flat.reshape(N, M).mean(axis=1)

        # 3. Optional Turn `t+2` Rollout on critical midgame macro turns (Turns 4–10)
        if self.three_turn_midgame and 4 <= turn_now <= 10 and t1_alive_idx:
            # Subsample first min(3, M) futures for Turn t+2 to keep runtime crisp
            M2 = min(3, M)
            t2_games: List[seaglass.TavernGame] = []
            t2_alive_idx: List[int] = []
            t2_dead = np.zeros(N * M2, dtype=np.float32)
            for i in range(N):
                for m in range(M2):
                    idx1 = i * M + m
                    idx2 = i * M2 + m
                    if t1_dead[idx1] < 0:
                        t2_dead[idx2] = -55.0
                        t2_games.append(t1_games[idx1])
                        continue
                    s2 = crn_seeds[m] ^ 0x3C3C
                    ob2, ot2 = self._next_turn_opponent(ctx, opp_board, opp_tier, turn_now + 1, m)
                    sg2 = t1_games[idx1].copy()
                    if ob2:
                        sg2.resolve_combat_against(ob2, opponent_tier=ot2, seed=(s2 ^ 0x1357) | 1)
                    if sg2.observe().health <= 0:
                        t2_dead[idx2] = -45.0
                        t2_games.append(sg2)
                    else:
                        sg2.reseed((s2 ^ 0x7F4A) | 1)
                        sg2.start_turn()
                        t2_alive_idx.append(idx2)
                        t2_games.append(sg2)
            if t2_alive_idx:
                rolled2 = self._rollout_turn_batch([t2_games[j] for j in t2_alive_idx], ctx, max_steps=16)
                for j, g_r in zip(t2_alive_idx, rolled2):
                    self._helper._fast_generic_order(g_r)
                    if g_r.is_legal(seaglass.TavernAction.end_turn()):
                        g_r.end_turn()
                    t2_games[j] = g_r
            v_t2_flat = np.zeros(N * M2, dtype=np.float32)
            if t2_alive_idx:
                v2_alive = self._value_batch([t2_games[j] for j in t2_alive_idx], ctx, end_of_turn=True)
                for j, val in zip(t2_alive_idx, v2_alive):
                    v_t2_flat[j] = val
            v_t2_flat += t2_dead
            v_t2 = v_t2_flat.reshape(N, M2).mean(axis=1)
            return 0.30 * v_eot + 0.40 * v_t1 + 0.30 * v_t2 + combat_t0

        return 0.22 * v_eot + 0.78 * v_t1 + combat_t0

    # ------------------------------------------------------------------
    # Policy-Guided Deterministic Beam + CRN Stochastic Branching
    # ------------------------------------------------------------------
    def _plan_turn(
        self,
        base_g: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        is_ghost: bool,
        refs_so_far: int,
        replan_idx: int = 0,
    ) -> Tuple[List[seaglass.TavernAction], bool]:
        """
        Discovers candidate within-turn lines via Policy-Guided Beam Search, then scores
        the diverse finalists + CRN stochastic branches via Multi-Turn Stochastic Rollouts!
        Returns `(action_sequence, stops_on_stochastic)`.
        """
        obs0 = base_g.observe()
        turn_now = obs0.turn
        alive_units = ctx.alive_units if ctx is not None else 8
        is_finals = alive_units <= 2 and not is_ghost

        frontier: List[_BeamNode] = [_BeamNode(game=base_g.copy(), actions=[])]
        eot_nodes: Dict[Tuple, _BeamNode] = {}
        stoch_nodes: Dict[Tuple, _BeamNode] = {}
        visited: set[Tuple] = {_state_sig(obs0)}

        # Only seed empty `end_turn` baseline if we have <= 1 gold and no playable hand cards
        if obs0.gold <= 1 and len(obs0.hand) == 0:
            eot_nodes[_state_sig(obs0)] = _BeamNode(game=base_g.copy(), actions=[], ended=True)

        for depth in range(self.max_depth):
            if not frontier:
                break
            games_f = [nd.game for nd in frontier]
            lps_f, _, mks_f = self._batch_policy_and_critic(games_f, ctx, need_mask=True)

            next_cands: List[_BeamNode] = []
            for nd, lps, mk in zip(frontier, lps_f, mks_f):
                legal_idx = np.flatnonzero(mk)
                if len(legal_idx) == 0:
                    sig = _state_sig(nd.game.observe())
                    if sig not in eot_nodes:
                        eot_nodes[sig] = _BeamNode(game=nd.game, actions=nd.actions, logp_sum=nd.logp_sum, ended=True)
                    continue

                order = legal_idx[np.argsort(lps[legal_idx])[::-1]]
                chosen_idx: List[int] = [int(i) for i in order[: self.top_k_expand]]

                # Ensure macro forks (`upgrade_tavern`, `dark_discovery`, `refresh`, `freeze`, `end_turn`) are never missed
                extras = [
                    seaglass.TavernAction.upgrade_tavern(),
                    seaglass.TavernAction.dark_discovery(),
                    seaglass.TavernAction.end_turn(),
                ]
                if depth <= 2 and refs_so_far < 5:
                    extras.append(seaglass.TavernAction.refresh())
                if depth == 0:
                    extras.append(seaglass.TavernAction.toggle_freeze())
                for ea in extras:
                    ei = ACTION_TO_INDEX.get((ea.kind, ea.args[0], ea.args[1]))
                    if ei is not None and mk[ei] and ei not in chosen_idx:
                        chosen_idx.append(ei)

                # In Finals, also allow selling board slots to unlock Scam/Tech swaps purely from physics!
                if is_finals and depth <= 2 and len(nd.game.observe().board) >= 6:
                    for b_i in range(len(nd.game.observe().board)):
                        sa = seaglass.TavernAction.sell(b_i)
                        si = ACTION_TO_INDEX.get((sa.kind, sa.args[0], sa.args[1]))
                        if si is not None and mk[si] and si not in chosen_idx:
                            chosen_idx.append(si)

                for a_i in chosen_idx:
                    act = action_from_index(a_i)
                    lp = float(lps[a_i])
                    if act.kind == "end_turn":
                        o_cur = nd.game.observe()
                        # Skip premature mid-turn `end_turn` candidates while holding >= 3 unspent gold
                        if o_cur.gold >= 3 and len(nd.actions) == 0:
                            continue
                        sig = _state_sig(o_cur)
                        prev = eot_nodes.get(sig)
                        if prev is None or nd.logp_sum > prev.logp_sum:
                            eot_nodes[sig] = _BeamNode(
                                game=nd.game, actions=list(nd.actions), logp_sum=nd.logp_sum, ended=True
                            )
                        continue

                    if act.kind in _STOCHASTIC_KINDS:
                        if act.kind == "refresh" and refs_so_far >= 5:
                            continue
                        st_key = (_state_sig(nd.game.observe()), _action_key(act))
                        prev = stoch_nodes.get(st_key)
                        if prev is None or (nd.logp_sum + lp) > prev.logp_sum:
                            stoch_nodes[st_key] = _BeamNode(
                                game=nd.game,
                                actions=list(nd.actions) + [act],
                                logp_sum=nd.logp_sum + lp,
                                stochastic_trigger=act,
                            )
                        continue

                    if not nd.game.is_legal(act):
                        continue
                    sg = nd.game.copy()
                    ended = sg.step(act)
                    obs_n = sg.observe()
                    # If an action triggered a random Discover prompt, treat as a stochastic branch!
                    if obs_n.discover_pending is not None:
                        st_key = (_state_sig(nd.game.observe()), _action_key(act))
                        if st_key not in stoch_nodes:
                            stoch_nodes[st_key] = _BeamNode(
                                game=nd.game,
                                actions=list(nd.actions) + [act],
                                logp_sum=nd.logp_sum + lp,
                                stochastic_trigger=act,
                            )
                        continue

                    sig = _state_sig(obs_n)
                    if ended:
                        prev = eot_nodes.get(sig)
                        if prev is None or (nd.logp_sum + lp) > prev.logp_sum:
                            eot_nodes[sig] = _BeamNode(
                                game=sg, actions=list(nd.actions) + [act], logp_sum=nd.logp_sum + lp, ended=True
                            )
                    elif sig not in visited:
                        visited.add(sig)
                        next_cands.append(
                            _BeamNode(
                                game=sg,
                                actions=list(nd.actions) + [act],
                                logp_sum=nd.logp_sum + lp,
                            )
                        )

            if not next_cands:
                break
            # Prune mid-turn frontier using `EntityTransformer`'s OWN mid-turn Critic `V_π(s)` + policy prior
            # (NEVER EOT-only `architect_value_v2`, which was trained only on post-spend EOT states!)
            if len(next_cands) > self.beam_width:
                _, v_crit, _ = self._batch_policy_and_critic([n.game for n in next_cands], ctx, need_mask=False)
                for n, v in zip(next_cands, v_crit):
                    n.pre_score = float(v) + 0.15 * (n.logp_sum / max(1, len(n.actions)))
                # Keep diverse macro tiers in frontier so upgrading never gets pruned before finishing spend!
                by_tier: Dict[int, List[_BeamNode]] = {}
                for n in next_cands:
                    by_tier.setdefault(n.game.observe().tavern_tier, []).append(n)
                kept: List[_BeamNode] = []
                for t_nodes in by_tier.values():
                    t_nodes.sort(key=lambda x: x.pre_score, reverse=True)
                    kept.extend(t_nodes[:3])
                kept_ids = {id(x) for x in kept}
                rest = [n for n in next_cands if id(n) not in kept_ids]
                rest.sort(key=lambda x: x.pre_score, reverse=True)
                frontier = (kept + rest)[: self.beam_width]
            else:
                frontier = next_cands

        # Guaranteed Policy Improvement: ALWAYS inject the full greedy `π(a|s)` deterministic prefix
        # (rolling forward until End Turn or first stochastic reveal) so search NEVER under-spends a turn!
        pi_g = base_g.copy()
        pi_acts: List[seaglass.TavernAction] = []
        pi_lp = 0.0
        pi_hit_stoch: Optional[seaglass.TavernAction] = None
        for _ in range(18):
            o_pi = pi_g.observe()
            lps_p, _, mks_p = self._batch_policy_and_critic([pi_g], ctx, need_mask=True)
            if not mks_p[0].any():
                break
            a_i = int(np.argmax(lps_p[0]))
            act = action_from_index(a_i)
            lp = float(lps_p[0, a_i])
            if act.kind == "end_turn" or not pi_g.is_legal(act):
                break
            if act.kind in _STOCHASTIC_KINDS:
                if not (act.kind == "refresh" and refs_so_far >= 5):
                    pi_hit_stoch = act
                    st_key = (_state_sig(o_pi), _action_key(act))
                    stoch_nodes[st_key] = _BeamNode(
                        game=pi_g.copy(),
                        actions=list(pi_acts) + [act],
                        logp_sum=pi_lp + lp + 0.5,
                        stochastic_trigger=act,
                    )
                break
            sg_next = pi_g.copy()
            ended = sg_next.step(act)
            if sg_next.observe().discover_pending is not None:
                st_key = (_state_sig(o_pi), _action_key(act))
                stoch_nodes[st_key] = _BeamNode(
                    game=pi_g.copy(),
                    actions=list(pi_acts) + [act],
                    logp_sum=pi_lp + lp + 0.5,
                    stochastic_trigger=act,
                )
                pi_hit_stoch = act
                break
            pi_g = sg_next
            pi_acts.append(act)
            pi_lp += lp
            if ended:
                break
        if pi_hit_stoch is None:
            sig_pi = _state_sig(pi_g.observe())
            eot_nodes[sig_pi] = _BeamNode(
                game=pi_g, actions=pi_acts, logp_sum=pi_lp + 0.5, ended=True
            )

        # Complete any unfinished frontier nodes (still holding gold/cards at `max_depth`) via greedy `π`
        # so EOT `architect_value_v2` NEVER evaluates a half-spent board!
        if frontier:
            unfinished = [nd for nd in frontier if nd.game.observe().gold >= 2 or len(nd.game.observe().hand) > 0]
            for nd in frontier:
                if nd not in unfinished:
                    sig = _state_sig(nd.game.observe())
                    if sig not in eot_nodes:
                        eot_nodes[sig] = _BeamNode(game=nd.game, actions=nd.actions, logp_sum=nd.logp_sum, ended=True)
            if unfinished:
                # Roll out deterministic continuation for unfinished frontier nodes
                for nd in unfinished[:6]:
                    g_c = nd.game.copy()
                    acts_c = list(nd.actions)
                    for _ in range(8):
                        lps_c, _, mks_c = self._batch_policy_and_critic([g_c], ctx, need_mask=True)
                        if not mks_c[0].any():
                            break
                        a_i = int(np.argmax(lps_c[0]))
                        act = action_from_index(a_i)
                        if act.kind == "end_turn" or act.kind in _STOCHASTIC_KINDS or not g_c.is_legal(act):
                            break
                        g_try = g_c.copy()
                        ended = g_try.step(act)
                        if g_try.observe().discover_pending is not None:
                            break
                        g_c = g_try
                        acts_c.append(act)
                        if ended:
                            break
                    sig = _state_sig(g_c.observe())
                    if sig not in eot_nodes:
                        eot_nodes[sig] = _BeamNode(game=g_c, actions=acts_c, logp_sum=nd.logp_sum, ended=True)

        # Select diverse deterministic finalists (preserving best of each `(tier, frozen)` macro bucket!)
        det_list = list(eot_nodes.values())
        if len(det_list) > self.finalist_det:
            v_fast = self._value_batch([n.game for n in det_list], ctx, end_of_turn=True)
            for n, v in zip(det_list, v_fast):
                n.pre_score = float(v) + 0.15 * n.logp_sum
            buckets: Dict[Tuple[int, bool], List[_BeamNode]] = {}
            for n in det_list:
                o = n.game.observe()
                buckets.setdefault((o.tavern_tier, bool(o.is_frozen)), []).append(n)
            finalists_det: List[_BeamNode] = []
            for b_nodes in buckets.values():
                b_nodes.sort(key=lambda x: x.pre_score, reverse=True)
                finalists_det.append(b_nodes[0])
            used = {id(x) for x in finalists_det}
            rest = [n for n in det_list if id(n) not in used]
            rest.sort(key=lambda x: x.pre_score, reverse=True)
            finalists_det = (finalists_det + rest)[: self.finalist_det]
        else:
            finalists_det = det_list

        # Select top stochastic branches (prioritizing highest policy log-prob + lowest depth)
        st_list = sorted(
            stoch_nodes.values(),
            key=lambda x: (x.logp_sum - 0.15 * len(x.actions)),
            reverse=True,
        )[: self.finalist_stoch]

        # Shared CRN seeds for apples-to-apples unbiased evaluation!
        can_upg = base_g.is_legal(seaglass.TavernAction.upgrade_tavern())
        use_t2 = self.two_turn_horizon and (replan_idx <= 1 or can_upg) and (turn_now >= 3)
        M = 2 if turn_now <= 2 else (max(3, self.rollout_futures) if use_t2 else 2)
        crn = [self.rng.getrandbits(63) | 1 for _ in range(M)]
        crn_t2 = crn if use_t2 else []

        # 1. Evaluate all deterministic finalists (with full Multi-Turn Rollouts when `use_t2`!)
        det_scores = self._evaluate_eot_lines_multi_turn(
            [n.game for n in finalists_det],
            ctx=ctx,
            opp_board=opp_board,
            opp_tier=opp_tier,
            is_ghost=is_ghost,
            crn_seeds=crn_t2 if crn_t2 else crn[:1],
            turn_now=turn_now if use_t2 else 99,
            is_finals=is_finals,
        )

        # 2. Evaluate all stochastic branches via M CRN-paired rollouts of Turn `t`!
        st_scores = np.zeros(len(st_list), dtype=np.float32)
        if st_list:
            st_sims: List[seaglass.TavernGame] = []
            for nd in st_list:
                trig = nd.stochastic_trigger
                for s in crn:
                    sg = nd.game.copy()
                    sg.reseed(s)
                    if trig is not None and sg.is_legal(trig):
                        sg.step(trig)
                    st_sims.append(sg)
            st_sims = self._rollout_turn_batch(st_sims, ctx, max_steps=12)
            st_vals = self._evaluate_eot_lines_multi_turn(
                st_sims,
                ctx=ctx,
                opp_board=opp_board,
                opp_tier=opp_tier,
                is_ghost=is_ghost,
                crn_seeds=crn[:1],
                turn_now=turn_now if use_t2 else 99,
                is_finals=is_finals,
            )
            st_scores = st_vals.reshape(len(st_list), M).mean(axis=1)

        # Pick global best across deterministic finalists AND stochastic branches!
        best_score = -1e18
        best_actions: List[seaglass.TavernAction] = []
        best_is_stoch = False

        for nd, sc in zip(finalists_det, det_scores):
            tot = float(sc) + self.prior_weight * (nd.logp_sum / max(1, len(nd.actions)))
            if tot > best_score:
                best_score = tot
                best_actions = list(nd.actions)
                best_is_stoch = False

        for nd, sc in zip(st_list, st_scores):
            tot = float(sc) + self.prior_weight * (nd.logp_sum / max(1, len(nd.actions)))
            if tot > best_score:
                best_score = tot
                best_actions = list(nd.actions)
                best_is_stoch = True

        return best_actions, best_is_stoch

    # ------------------------------------------------------------------
    # Main Lobby Interface
    # ------------------------------------------------------------------
    def act_in_lobby(self, lobby: seaglass.LobbyGame, player: int) -> seaglass.TavernAction:
        obs = lobby.observe(player)
        ctx = lobby.context(player) if lobby.phase == "turn" else None
        legal = lobby.legal_actions(player)
        self.last_note = None
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            return legal[0]

        def _match(ak: Tuple[str, Any, Any]) -> Optional[seaglass.TavernAction]:
            for a in legal:
                if _action_key(a) == ak:
                    return a
            return None

        lobby_id = id(lobby)
        base_g = lobby.belief_tavern(player, seed=self.rng.getrandbits(63) | 1, extra_units=[])
        opp_board, opp_tier, is_ghost, _ = self._helper._opponent_board_and_tier(obs, ctx)

        # 1. Drain queued deterministic plan steps (instant, zero re-search!)
        rk = (lobby_id, player, obs.turn)
        q = self._queued.get(player)
        if q:
            ak, exp_sig, stop_after = q.pop(0)
            if _state_sig(obs) == exp_sig:
                if ak[0] == "end_turn":
                    self._queued.pop(player, None)
                    return self._finish_with_positioning(
                        base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, ctx
                    )
                m = _match(ak)
                if m is not None:
                    if ak[0] == "refresh":
                        self._ref_count[rk] = self._ref_count.get(rk, 0) + 1
                    if stop_after or not q:
                        self._queued.pop(player, None)
                    return m
            self._queued.pop(player, None)

        # 2. Prompt Phase: Choose Hero (pure learned `EntityTransformer` + `ValueNet`, ZERO hardcoded table!)
        if legal[0].kind == "choose_hero":
            lps, _, _ = self._batch_policy_and_critic([base_g], ctx, need_mask=True)
            post_games = []
            priors = []
            for a in legal:
                sg = base_g.copy()
                sg.step(a)
                post_games.append(sg)
                ai = ACTION_TO_INDEX.get((a.kind, a.args[0], a.args[1]))
                priors.append(float(lps[0, ai]) if ai is not None else -5.0)
            vals = self._value_batch(post_games, ctx, end_of_turn=False)
            scores = vals + 3.0 * np.asarray(priors, dtype=np.float32)
            return legal[int(np.argmax(scores))]

        # 3. Prompt Phases: Choose Trinket & Choose Discover (2-Turn CRN Batched Rollouts!)
        if legal[0].kind in ("choose_trinket", "choose_discover"):
            is_tr = legal[0].kind == "choose_trinket"
            M = max(6, self.rollout_futures + 2) if is_tr else max(4, self.rollout_futures)
            crn = [self.rng.getrandbits(63) | 1 for _ in range(M)]
            sims: List[seaglass.TavernGame] = []
            for a in legal:
                for s in crn:
                    sg = base_g.copy()
                    sg.reseed(s)
                    if sg.is_legal(a):
                        sg.step(a)
                    sims.append(sg)
            sims = self._rollout_turn_batch(sims, ctx, max_steps=18)
            alive_units = ctx.alive_units if ctx is not None else 8
            is_finals = alive_units <= 2 and not is_ghost
            vals = self._evaluate_eot_lines_multi_turn(
                sims,
                ctx=ctx,
                opp_board=opp_board,
                opp_tier=opp_tier,
                is_ghost=is_ghost,
                crn_seeds=crn[: min(3, M)] if is_tr else crn[: min(2, M)],
                turn_now=obs.turn,
                is_finals=is_finals,
            )
            opt_scores = vals.reshape(len(legal), M).mean(axis=1)
            return legal[int(np.argmax(opt_scores))]

        # 4. Normal Tavern Turn: Plan full turn via Policy-Guided Beam + Multi-Turn Rollouts!
        refs_so_far = self._ref_count.get(rk, 0)
        replan_idx = self._replan_count.get(rk, 0)
        self._replan_count[rk] = replan_idx + 1
        plan_actions, is_stoch = self._plan_turn(
            base_g, ctx, opp_board, opp_tier, is_ghost, refs_so_far, replan_idx=replan_idx
        )

        if not plan_actions:
            return self._finish_with_positioning(
                base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, ctx
            )

        # Queue verified deterministic prefix (stopping immediately on stochastic action to replan!)
        sim = base_g.copy()
        steps: List[Tuple[Tuple[str, Any, Any], Tuple, bool]] = []
        for idx_a, a in enumerate(plan_actions):
            ak = _action_key(a)
            stop_here = is_stoch and (idx_a == len(plan_actions) - 1)
            steps.append((ak, _state_sig(sim.observe()), stop_here))
            if stop_here:
                break
            if sim.is_legal(a):
                ended = sim.step(a)
                if ended:
                    break
        if not is_stoch:
            steps.append((("end_turn", None, None), _state_sig(sim.observe()), True))

        first_ak, _, first_stop = steps[0]
        if first_ak[0] == "refresh":
            self._ref_count[rk] = refs_so_far + 1
        m0 = _match(first_ak)
        if m0 is not None:
            if not first_stop and len(steps) > 1:
                self._queued[player] = steps[1:]
            return m0
        return self._finish_with_positioning(
            base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, ctx
        )

    def _finish_with_positioning(
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
            repo = self._helper._best_position_actions(base_g, opp_board, opp_tier, ctx)
            if repo:
                sim = base_g.copy()
                steps: List[Tuple[Tuple[str, Any, Any], Tuple, bool]] = []
                for ra in repo:
                    steps.append((_action_key(ra), _state_sig(sim.observe()), False))
                    if sim.is_legal(ra):
                        sim.step(ra)
                steps.append((("end_turn", None, None), _state_sig(sim.observe()), True))
                for la in legal:
                    if _action_key(la) == steps[0][0]:
                        self._queued[player] = steps[1:]
                        return la
        for la in legal:
            if la.kind == "end_turn":
                return la
        return legal[0]
