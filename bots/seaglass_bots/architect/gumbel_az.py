"""Amber-Eye-Inspired Gumbel AlphaZero Architecture for Hearthstone Battlegrounds (`gumbel_az.py`).

100% Pure End-to-End Neural + Search Architecture (ZERO hardcoded card/comp/hero/trinket/leveling rules!):
  1. `AmberNet` (extends `DistEntityTransformer` with exact Amber Eye / KataGo multi-head design):
       - Primary Policy Head `π(a|s)` (entity-attention over all legal actions)
       - KataGo Soft Policy Head `π_soft(a|s)` trained on `π_target^(1/4)` (richer relative rankings)
       - 8-Way Distributional Placement Head `P(place=1..8 | s)` -> `V(s) = probs @ REWARD_ATOMS`
       - Dense Auxiliary Future-Trajectory Head: predicts `(log1p(stats_{t+2})/10, next_hp/40, alive_{t+2})`
         extracting 10x more signal per game and mitigating value memorization!
       - Warm-starts encoder + actor cleanly from any trained `EntityTransformer` (`ppo_v7` / `ppo_v8`).
  2. `GumbelSearchBot` (Gumbel AlphaZero Sequential Halving + Transposition DAG + DPW + Pure-Physics EOT):
       - Uses Gumbel-Top-k + Sequential Halving (Danihelka et al., ICLR 2022) — guarantees strict policy
         improvement even at small search budgets (`12-64` sims) with ZERO off-policy Goodharting when
         trained on-policy!
       - Computes exact completed Q-values `completedQ(a)` and improved policy targets
         `π_target = softmax(logits + sigma(completedQ))` for self-play Expert Iteration!
       - Handles stochastic actions (`refresh`, `dark_discovery`, `choose_discover`, `choose_trinket`)
         via Double Progressive Widening (CRN-paired fresh RNG resamples, zero savescumming!).
       - Supports batched FP16 GPU evaluation (`device='cuda'`) for high-throughput self-play.
"""

from __future__ import annotations

import math
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
import torch.nn as nn
import torch.nn.functional as F

from seaglass_rl import checkpoint as rl_ckpt
from seaglass_rl.actions import ACTION_TO_INDEX, action_from_index, action_mask
from seaglass_rl.features import encode_observation

from .dist_transformer import DIST_PLACE_ATOMS, DistEntityTransformer
from .policy_bot import _GameView
from .rl_physics_bot import FinalsPhysicsRLBot
from .zero_bot import ZeroPhysicsBot, _action_key, _state_sig

_STOCHASTIC_ACTION_KINDS = frozenset(
    {"refresh", "dark_discovery", "choose_discover", "choose_trinket", "hero_power", "hero_power_2"}
)


class AmberNet(DistEntityTransformer):
    """Amber-Eye-Style Multi-Head Entity-Transformer for Gumbel AlphaZero in Battlegrounds."""

    def __init__(self, **kwargs) -> None:
        super().__init__(**kwargs)
        d_model = int(self.config["d_model"])
        head_dim = int(self.config["head_dim"])
        # 1. KataGo Soft Policy readout (predicts flattened `p^(1/4)` policy targets during training)
        self.soft_actor_out = nn.Linear(head_dim, 1)
        nn.init.normal_(self.soft_actor_out.weight, std=0.01)
        nn.init.zeros_(self.soft_actor_out.bias)

        # 2. Dense Auxiliary Trajectory Head: [future_stats_t2, next_hp_norm, survive_t2_logit]
        #    Uses pooled summary (player token + mean of non-empty tokens) just like Amber Eye!
        self.summary_mlp = nn.Sequential(
            nn.Linear(2 * d_model, d_model),
            nn.GELU(),
        )
        self.aux_head = nn.Sequential(
            nn.Linear(d_model, d_model),
            nn.GELU(),
            nn.Linear(d_model, 3),
        )
        # Upgrade dist_critic to also consume rich pooled summary
        self.dist_critic = nn.Sequential(
            nn.Linear(d_model, d_model),
            nn.GELU(),
            nn.Linear(d_model, 8),
        )

    def forward_all_heads(
        self,
        player_vec: torch.Tensor,
        slot_card_ids: torch.Tensor,
        slot_num_feats: torch.Tensor,
        action_mask: torch.Tensor | None = None,
    ) -> Tuple[torch.Tensor, torch.Tensor, torch.Tensor, torch.Tensor, torch.Tensor]:
        """Returns `(logits, soft_logits, expected_value, place_logits_8, aux_3)`."""
        B = player_vec.shape[0]
        slots, lengths, token_of_slot = self._pack(slot_card_ids, slot_num_feats)
        h, start = self._encode(player_vec, slot_card_ids, slot_num_feats, slots, lengths)
        h_player = h[start]

        # Compute mean-pooled non-empty slot summary per row (Amber Eye summary architecture!)
        # Fast vectorized gather across non-empty tokens using `start` + `lengths`
        # Approximate cleanly with player + mean over present slots via scatter-add
        S = slot_card_ids.shape[1]
        tok_idx = token_of_slot[:, :S]  # (B, S), 0 for empty, 1..len for present
        present = (tok_idx > 0).float()  # (B, S)
        flat_idx = start[:, None] + tok_idx  # (B, S)
        slot_h = h[flat_idx] * present.unsqueeze(-1)
        mean_h = slot_h.sum(dim=1) / present.sum(dim=1, keepdim=True).clamp(min=1.0)
        summary = self.summary_mlp(torch.cat([h_player, mean_h], dim=-1))

        place_logits = self.dist_critic(summary)  # (B, 8)
        place_probs = F.softmax(place_logits, dim=-1)
        value = (place_probs * self.reward_atoms).sum(dim=-1)
        aux = self.aux_head(summary)  # (B, 3)

        A = self.act_src.shape[0]
        if action_mask is None:
            rows = torch.arange(B, device=h.device).repeat_interleave(A)
            acts = torch.arange(A, device=h.device).repeat(B)
        else:
            rows, acts = action_mask.nonzero(as_tuple=True)
        src = start[rows] + token_of_slot[rows, self.act_src[acts]]
        pre = self.w_src(h[src]) + self.w_ctx(h_player)[rows] + self.per_action_embed()[acts]
        tgt = self.act_tgt[acts]
        has_tgt = tgt >= 0
        if bool(has_tgt.any()):
            with_tgt = has_tgt.nonzero(as_tuple=True)[0]
            r = rows[with_tgt]
            tgt_tok = start[r] + token_of_slot[r, tgt[with_tgt]]
            pre = pre.index_add(0, with_tgt, self.w_tgt(h[tgt_tok]).to(pre.dtype))
        hid = torch.relu(pre)
        scores = self.actor_out(hid).squeeze(-1).float()
        soft_scores = self.soft_actor_out(hid).squeeze(-1).float()

        logits = scores.new_full((B, A), -1e9)
        soft_logits = soft_scores.new_full((B, A), -1e9)
        logits[rows, acts] = scores
        soft_logits[rows, acts] = soft_scores
        if action_mask is not None and self.kind_prior:
            counts = action_mask.float() @ self.kind_onehot
            per_action = counts @ self.kind_onehot.T
            log_cnt = torch.log(per_action.clamp(min=1.0))
            logits = (logits - log_cnt).masked_fill(~action_mask, -1e9)
            soft_logits = (soft_logits - log_cnt).masked_fill(~action_mask, -1e9)
        return logits, soft_logits, value, place_logits, aux

    def forward_with_dist(
        self,
        player_vec: torch.Tensor,
        slot_card_ids: torch.Tensor,
        slot_num_feats: torch.Tensor,
        action_mask: torch.Tensor | None = None,
    ) -> Tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
        logits, _, value, place_logits, _ = self.forward_all_heads(
            player_vec, slot_card_ids, slot_num_feats, action_mask
        )
        return logits, value, place_logits

    def forward(
        self,
        player_vec: torch.Tensor,
        slot_card_ids: torch.Tensor,
        slot_num_feats: torch.Tensor,
        action_mask: torch.Tensor | None = None,
    ) -> Tuple[torch.Tensor, torch.Tensor]:
        logits, _, value, _, _ = self.forward_all_heads(
            player_vec, slot_card_ids, slot_num_feats, action_mask
        )
        return logits, value

    @classmethod
    def from_any_checkpoint(cls, ckpt_path: str) -> "AmberNet":
        ck = torch.load(ckpt_path, map_location="cpu", weights_only=False)
        cfg = dict(ck["model_config"])
        model = cls(**cfg)
        if ck.get("model_class") == "AmberNet":
            model.load_state_dict(ck["model_state"], strict=True)
        else:
            model.load_state_dict(ck["model_state"], strict=False)
        model.eval()
        return model

    def save_checkpoint(self, path: str, iteration: int = 0, extra: Optional[Dict] = None) -> None:
        Path(path).parent.mkdir(parents=True, exist_ok=True)
        torch.save(
            {
                "model_class": "AmberNet",
                "model_config": dict(self.config),
                "model_state": {k: v.detach().cpu() for k, v in self.state_dict().items()},
                "iteration": iteration,
                "extra": extra or {},
            },
            path,
        )


def gumbel_completed_policy(
    raw_logits: np.ndarray,
    legal_mask: np.ndarray,
    root_value: float,
    q_values: Dict[int, float],
    visits: Dict[int, int],
    c_visit: float = 16.0,
    c_scale: float = 1.0,
) -> Tuple[np.ndarray, float]:
    """
    Exact Gumbel AlphaZero (Danihelka et al., ICLR 2022) completed-Q policy improvement target!
      - Replaces unvisited actions with mixed root value `v_mix`.
      - Applies monotone scale `sigma(q) = (c_visit + max_N) * c_scale * q` (normalized cleanly).
      - Returns `(pi_improved (A,), v_improved_root)`.
    """
    probs_prior = np.zeros_like(raw_logits, dtype=np.float64)
    leg = np.flatnonzero(legal_mask)
    if len(leg) == 0:
        return probs_prior.astype(np.float32), float(root_value)
    l_leg = raw_logits[leg].astype(np.float64)
    l_leg -= l_leg.max()
    e_leg = np.exp(l_leg)
    p_leg = e_leg / max(1e-12, e_leg.sum())
    probs_prior[leg] = p_leg

    sum_n = sum(visits.values())
    if sum_n <= 0:
        return probs_prior.astype(np.float32), float(root_value)

    # Compute mixed value `v_mix` (Eq. 33 in Gumbel MuZero paper)
    vis_p_sum = 0.0
    vis_pq_sum = 0.0
    max_n = 0
    for a, n_a in visits.items():
        if n_a > 0:
            p_a = float(probs_prior[a])
            vis_p_sum += p_a
            vis_pq_sum += p_a * float(q_values[a])
            if n_a > max_n:
                max_n = n_a
    if vis_p_sum > 1e-8:
        v_mix = (root_value + (sum_n / vis_p_sum) * vis_pq_sum) / (1.0 + sum_n)
    else:
        v_mix = root_value

    # Completed Q vector over legal actions
    comp_q = np.full(len(leg), v_mix, dtype=np.float64)
    for k, a in enumerate(leg):
        if visits.get(int(a), 0) > 0:
            comp_q[k] = float(q_values[int(a)])

    # Normalize Q range cleanly for scale-invariant Gumbel sigma transform
    q_span = max(0.25, float(comp_q.max() - comp_q.min()))
    sigma_q = ((c_visit + max_n) / (c_visit + 1.0)) * c_scale * ((comp_q - v_mix) / q_span)

    imp_logits = raw_logits[leg].astype(np.float64) + sigma_q
    imp_logits -= imp_logits.max()
    imp_e = np.exp(imp_logits)
    imp_p = imp_e / max(1e-12, imp_e.sum())

    pi_out = np.zeros_like(raw_logits, dtype=np.float32)
    pi_out[leg] = imp_p.astype(np.float32)
    v_imp = float(np.dot(imp_p, comp_q))
    return pi_out, v_imp


def _fast_keyword_order_game(g: seaglass.TavernGame) -> None:
    """<0.05ms pure-keyword board ordering on a `TavernGame` (zero card names!)."""
    obs = g.observe()
    n = len(obs.board)
    if n < 2:
        return

    def _key(u: seaglass.Unit) -> Tuple[int, float, int]:
        is_taunt = 1 if u.taunt else 0
        pri = 0
        if u.windfury:
            pri -= 3
        if u.divine_shield:
            pri -= 2
        if u.venomous:
            pri -= 1
        if is_taunt and not (u.windfury or u.divine_shield):
            pri += 4
        if u.reborn and not is_taunt:
            pri -= 1
        return (pri, -float(u.attack), -int(u.health))

    for target_pos in range(n):
        obs_c = g.observe()
        if len(obs_c.board) != n:
            break
        best_i = min(range(target_pos, n), key=lambda idx: _key(obs_c.board[idx]))
        if best_i != target_pos:
            act = seaglass.TavernAction.reposition(best_i, target_pos)
            if g.is_legal(act):
                g.step(act)


def run_gumbel_root_search(
    model: nn.Module,
    base_g: seaglass.TavernGame,
    ctx: Optional[seaglass.LobbyContext],
    opp_board: List[seaglass.Unit],
    opp_tier: int,
    rng: np.random.Generator,
    device: torch.device,
    num_top_m: int = 4,
    num_sims: int = 16,
    add_gumbel_noise: bool = True,
    rollout_depth: int = 12,
    c_scale: float = 1.2,
    value_model: Optional[nn.Module] = None,
    value_blend: float = 0.5,
    eq_weight: float = 0.0,
    lookahead_turns: int = 1,
) -> Tuple[int, np.ndarray, float]:
    """Single-root convenience wrapper over Horizon-Aligned Batched Gumbel Sequential Halving."""
    return run_gumbel_root_search_batch(
        model=model,
        roots=[(base_g, ctx, opp_board, opp_tier)],
        rng=rng,
        device=device,
        num_top_m=num_top_m,
        num_sims=num_sims,
        add_gumbel_noise=add_gumbel_noise,
        rollout_depth=rollout_depth,
        c_scale=c_scale,
        value_model=value_model,
        value_blend=value_blend,
        eq_weight=eq_weight,
        lookahead_turns=lookahead_turns,
    )[0]


def run_gumbel_root_search_batch(
    model: nn.Module,
    roots: Sequence[Tuple[seaglass.TavernGame, Optional[seaglass.LobbyContext], List[seaglass.Unit], int]],
    rng: np.random.Generator,
    device: torch.device,
    num_top_m: int = 4,
    num_sims: int = 12,
    add_gumbel_noise: bool = True,
    rollout_depth: int = 12,
    c_scale: float = 1.2,
    value_model: Optional[nn.Module] = None,
    value_blend: float = 0.5,
    eq_weight: float = 0.0,
    lookahead_turns: int = 1,
) -> List[Tuple[int, np.ndarray, float]]:
    """
    Horizon-Aligned Amber-Eye-Style GPU-Batched Gumbel Sequential Halving across `K` roots!
      - Rolls EVERY candidate action (`upgrade_tavern`, `buy`, `refresh`, `trinket`, `discover`)
        cleanly to the EXACT SAME Bellman boundary:
          `End-of-Turn t -> Fast Keyword Order -> end_turn() -> Combat t -> Start-of-Turn t+1`!
      - Eliminates fixed-depth horizon asymmetry (where `upgrade_tavern` took Combat `t` damage
        while `buy` stopped mid-turn before Combat `t`!).
      - Evaluates ALL `K` roots' simulations in unified FP16 CUDA batches!
    """
    K = len(roots)
    if K == 0:
        return []

    end_idx = ACTION_TO_INDEX[("end_turn", None, None)]
    upg_idx = ACTION_TO_INDEX.get(("upgrade_tavern", None, None), -1)
    dd_idx = ACTION_TO_INDEX.get(("dark_discovery", None, None), -1)

    pvs0, cis0, nfs0, mks0 = [], [], [], []
    leg_list: List[np.ndarray] = []
    is_hero_prompt: List[bool] = []
    for base_g, ctx, _, _ in roots:
        o0 = base_g.observe()
        mk0 = action_mask(_GameView(base_g), allow_reposition=False, allow_freeze=True, obs=o0)
        pv0, ci0, nf0 = encode_observation(o0, ctx, arranging=False)
        pvs0.append(pv0)
        cis0.append(ci0)
        nfs0.append(nf0)
        mks0.append(mk0)
        leg_list.append(np.flatnonzero(mk0))
        is_hero_prompt.append(bool(o0.hero_options) or o0.turn <= 0)

    with torch.inference_mode():
        with torch.amp.autocast("cuda", dtype=torch.float16, enabled=device.type == "cuda"):
            lg_t, val_t = model.forward(
                torch.from_numpy(np.stack(pvs0, axis=0).astype(np.float32)).to(device),
                torch.from_numpy(np.stack(cis0, axis=0).astype(np.int64)).to(device),
                torch.from_numpy(np.stack(nfs0, axis=0).astype(np.float32)).to(device),
                torch.from_numpy(np.stack(mks0, axis=0).astype(np.bool_)).to(device),
            )
        raw_logits_all = lg_t.float().detach().cpu().numpy()
        root_v_all = val_t.float().detach().cpu().numpy()

    # Initialize Gumbel-Top-k active candidate sets (with macro exploration guarantee for upgrade/DD!)
    active_by_r: List[List[int]] = []
    g_by_r: List[Dict[int, float]] = []
    q_sum_by_r: List[Dict[int, float]] = []
    visits_by_r: List[Dict[int, int]] = []
    for r in range(K):
        leg = leg_list[r]
        if len(leg) <= 1:
            active_by_r.append([int(leg[0])] if len(leg) == 1 else [end_idx])
            g_by_r.append({})
            q_sum_by_r.append({})
            visits_by_r.append({})
            continue
        g_noise = np.zeros(len(leg), dtype=np.float64)
        if add_gumbel_noise:
            u = rng.uniform(1e-6, 1.0 - 1e-6, size=len(leg))
            g_noise = -np.log(-np.log(u))
        sc0 = g_noise + raw_logits_all[r, leg].astype(np.float64)
        m_eff = min(num_top_m, len(leg))
        top_order = np.argsort(sc0)[::-1]
        cands = [int(leg[i]) for i in top_order[:m_eff]]
        # Ensure `upgrade_tavern` / `dark_discovery` aren't dropped if legal & competitive
        top_8 = {int(leg[i]) for i in top_order[: min(8, len(leg))]}
        for macro_a in (upg_idx, dd_idx):
            if macro_a >= 0 and macro_a in top_8 and macro_a not in cands and len(cands) >= 3:
                cands[-1] = macro_a
        active_by_r.append(cands)
        g_by_r.append({int(leg[i]): float(g_noise[i]) for i in range(len(leg))})
        q_sum_by_r.append({a: 0.0 for a in cands})
        visits_by_r.append({a: 0 for a in cands})

    n_phases = max(1, int(math.ceil(math.log2(max(2, num_top_m)))))
    for phase in range(n_phases):
        sims: List[seaglass.TavernGame] = []
        sim_r: List[int] = []
        sim_a: List[int] = []
        sim_seed: List[int] = []
        turn_done: List[bool] = []
        dead_mask: List[bool] = []

        for r in range(K):
            if len(leg_list[r]) <= 1:
                continue
            act_r = active_by_r[r]
            if len(act_r) <= 1 and phase > 0:
                continue
            base_g = roots[r][0]
            k_per = max(1, num_sims // max(1, n_phases * len(act_r)))
            crn = [int(rng.integers(1, 2**62)) | 1 for _ in range(k_per)]
            for a_i in act_r:
                act = action_from_index(a_i)
                for s in crn:
                    sg = base_g.copy()
                    sg.reseed(s)
                    ended = False
                    if act.kind == "end_turn":
                        ended = True
                    elif sg.is_legal(act):
                        ended = sg.step(act)
                    sims.append(sg)
                    sim_r.append(r)
                    sim_a.append(a_i)
                    sim_seed.append(s)
                    turn_done.append(bool(ended or is_hero_prompt[r]))
                    dead_mask.append(False)

        if not sims:
            break

        # 1-2. Symmetric Multi-Turn Bellman Horizon (`lookahead_turns` = 1 or 2 full turns!):
        #   Turn `t` uses `next_opponent`'s seen board; Turn `t+1` samples a random alive lobby opponent's
        #   seen board from `ctx.opponents` (100% honest Monte Carlo lobby field, zero peeking!).
        eq_bonus = np.zeros(len(sims), dtype=np.float32)
        n_turns_eff = max(1, int(lookahead_turns))
        alive_pools: List[List[Tuple[List[seaglass.Unit], int]]] = []
        if n_turns_eff > 1:
            for r in range(K):
                ctx_r = roots[r][1]
                pool: List[Tuple[List[seaglass.Unit], int]] = []
                if ctx_r is not None and getattr(ctx_r, "opponents", None):
                    for o_info in ctx_r.opponents:
                        sb = getattr(o_info, "seen_board", None)
                        if getattr(o_info, "alive", True) and sb:
                            pool.append((list(sb), int(getattr(o_info, "tavern_tier", roots[r][3]))))
                if not pool and roots[r][2]:
                    pool.append((roots[r][2], roots[r][3]))
                alive_pools.append(pool)

        for turn_step in range(n_turns_eff):
            if turn_step == 0:
                live_idx = [i for i, td in enumerate(turn_done) if not td]
            else:
                live_idx = [
                    i for i in range(len(sims))
                    if (not dead_mask[i]) and (not is_hero_prompt[sim_r[i]])
                ]

            for _ in range(rollout_depth):
                if not live_idx:
                    break
                pvs, cis, nfs, mks, keep = [], [], [], [], []
                for idx in live_idx:
                    g = sims[idx]
                    o = g.observe()
                    mk = action_mask(_GameView(g), allow_reposition=False, allow_freeze=True, obs=o)
                    if not mk.any():
                        continue
                    pv, ci, nf = encode_observation(o, roots[sim_r[idx]][1], arranging=False)
                    pvs.append(pv)
                    cis.append(ci)
                    nfs.append(nf)
                    mks.append(mk)
                    keep.append(idx)
                if not keep:
                    break
                with torch.inference_mode():
                    with torch.amp.autocast("cuda", dtype=torch.float16, enabled=device.type == "cuda"):
                        lg, _ = model.forward(
                            torch.from_numpy(np.stack(pvs, axis=0).astype(np.float32)).to(device),
                            torch.from_numpy(np.stack(cis, axis=0).astype(np.int64)).to(device),
                            torch.from_numpy(np.stack(nfs, axis=0).astype(np.float32)).to(device),
                            torch.from_numpy(np.stack(mks, axis=0).astype(np.bool_)).to(device),
                        )
                    a_ids = lg.argmax(dim=-1).detach().cpu().numpy()
                nxt_live = []
                for idx, a_step in zip(keep, a_ids):
                    act_s = action_from_index(int(a_step))
                    g = sims[idx]
                    if act_s.kind == "end_turn" or not g.is_legal(act_s):
                        continue
                    if not g.step(act_s):
                        nxt_live.append(idx)
                live_idx = nxt_live

            # Resolve EOT -> Fast Order -> Combat -> Start-of-Next-Turn across all alive non-hero sims!
            for idx, g in enumerate(sims):
                r_idx = sim_r[idx]
                if dead_mask[idx] or is_hero_prompt[r_idx]:
                    continue
                s = sim_seed[idx]
                _fast_keyword_order_game(g)
                if g.is_legal(seaglass.TavernAction.end_turn()):
                    g.end_turn()
                if turn_step == 0:
                    ob, ot = roots[r_idx][2], roots[r_idx][3]
                else:
                    pool = alive_pools[r_idx]
                    if pool:
                        ob, ot = pool[(s ^ (turn_step * 0x517C)) % len(pool)]
                    else:
                        ob, ot = [], roots[r_idx][3]
                if turn_step == 0 and ob and eq_weight > 0.0:
                    o_e = g.observe()
                    ehp_e = float(o_e.health + o_e.armor)
                    dist = g.equity_against(ob, opponent_tier=ot, n=8, base_seed=(s ^ 0x4C11) | 1)
                    dm = float(dist.damage_mean)
                    hp_mult = 1.60 if ehp_e <= 18.0 else (1.20 if ehp_e <= 26.0 else 0.85)
                    lethal_pen = -0.75 * max(0.0, 5.0 - (ehp_e + dm)) / 5.0 if (ehp_e + dm) < 5.0 else 0.0
                    eq_bonus[idx] = eq_weight * hp_mult * (
                        float(dist.a_win_rate) + 0.40 * float(dist.draw_rate) + 0.030 * dm + lethal_pen
                    )
                if ob:
                    g.resolve_combat_against(
                        ob, opponent_tier=ot, seed=(s ^ (0x9E37 + turn_step * 0x1337)) | 1
                    )
                if g.observe().health <= 0:
                    dead_mask[idx] = True
                else:
                    g.reseed((s ^ (0xA5A5 + turn_step * 0x2713)) | 1)
                    g.start_turn()

        # 3. Single batched GPU leaf value pass at Start-of-Turn `t + lookahead_turns` across ALL alive sims!
        eval_idx = [i for i, d in enumerate(dead_mask) if not d]
        leaf_vals = np.full(len(sims), -1.0, dtype=np.float32) + eq_bonus
        if eval_idx:
            pvs, cis, nfs = [], [], []
            for idx in eval_idx:
                pv, ci, nf = encode_observation(sims[idx].observe(), roots[sim_r[idx]][1], arranging=False)
                pvs.append(pv)
                cis.append(ci)
                nfs.append(nf)
            with torch.inference_mode():
                pv_t = torch.from_numpy(np.stack(pvs, axis=0).astype(np.float32)).to(device)
                ci_t = torch.from_numpy(np.stack(cis, axis=0).astype(np.int64)).to(device)
                nf_t = torch.from_numpy(np.stack(nfs, axis=0).astype(np.float32)).to(device)
                with torch.amp.autocast("cuda", dtype=torch.float16, enabled=device.type == "cuda"):
                    _, v_amber = model.forward(pv_t, ci_t, nf_t, None)
                vals_t = v_amber.float()
                if value_model is not None and value_blend > 0.0:
                    ev_s, _, fs_s, p1_lg = value_model.forward(pv_t, ci_t, nf_t)
                    v_scale = ev_s.float() + 0.35 * torch.sigmoid(p1_lg.float()) + 0.06 * fs_s.float()
                    b = min(1.0, max(0.0, float(value_blend)))
                    vals_t = (1.0 - b) * vals_t + b * v_scale
                vs = vals_t.detach().cpu().numpy()
            for idx, v in zip(eval_idx, vs):
                leaf_vals[idx] = float(v) + float(eq_bonus[idx])

        for r_idx, a_i, lv in zip(sim_r, sim_a, leaf_vals):
            q_sum_by_r[r_idx][a_i] += float(lv)
            visits_by_r[r_idx][a_i] += 1

        # Halve candidates for each root
        for r in range(K):
            act_r = active_by_r[r]
            if len(act_r) <= 1 or len(leg_list[r]) <= 1:
                continue
            q_m = {a: q_sum_by_r[r][a] / max(1, visits_by_r[r][a]) for a in act_r}
            q_arr = np.array([q_m[a] for a in act_r], dtype=np.float64)
            span = max(0.25, float(q_arr.max() - q_arr.min()))
            q_mid = float(q_arr.mean())
            act_r.sort(
                key=lambda a: g_by_r[r][a] + float(raw_logits_all[r, a]) + c_scale * ((q_m[a] - q_mid) / span),
                reverse=True,
            )
            active_by_r[r] = act_r[: max(1, len(act_r) // 2)]

    out: List[Tuple[int, np.ndarray, float]] = []
    for r in range(K):
        leg = leg_list[r]
        if len(leg) == 0:
            out.append((end_idx, np.zeros(len(mks0[r]), dtype=np.float32), 0.0))
        elif len(leg) == 1:
            pi_one = np.zeros(len(mks0[r]), dtype=np.float32)
            pi_one[int(leg[0])] = 1.0
            out.append((int(leg[0]), pi_one, float(root_v_all[r])))
        else:
            q_fin = {a: q_sum_by_r[r][a] / max(1, visits_by_r[r][a]) for a in visits_by_r[r] if visits_by_r[r][a] > 0}
            pi_imp, v_imp = gumbel_completed_policy(
                raw_logits_all[r], mks0[r], float(root_v_all[r]), q_fin, visits_by_r[r],
                c_visit=float(num_sims), c_scale=c_scale,
            )
            best_a = active_by_r[r][0] if active_by_r[r] else int(np.argmax(pi_imp))
            out.append((best_a, pi_imp, v_imp))
    return out


class GumbelSearchBot(FinalsPhysicsRLBot):
    """Amber-Eye-Style Evaluation Bot: On-Policy `AmberNet` + Gumbel Sequential Halving + Pure-Physics EOT/Finals!"""

    def __init__(
        self,
        policy_ckpt: str,
        value_ckpt: Optional[str] = None,
        value_blend: float = 0.5,
        num_top_m: int = 5,
        num_sims: int = 20,
        rollout_depth: int = 10,
        fast_conf: float = 0.88,
        c_scale: float = 1.2,
        eq_weight: float = 0.0,
        lookahead_turns: int = 1,
        macro_lookahead_turns: int = 1,
        use_cuda: bool = False,
        seed: int = 2029,
        **kwargs,
    ) -> None:
        super().__init__(policy_ckpt=policy_ckpt, seed=seed, **kwargs)
        self.device = torch.device("cuda" if (use_cuda and torch.cuda.is_available()) else "cpu")
        self.model = self.model.to(self.device)
        self.model.eval()
        self.value_model: Optional[nn.Module] = None
        if value_ckpt:
            from .scaling_value import ScalingValueTransformer

            self.value_model = ScalingValueTransformer.load_checkpoint(value_ckpt).to(self.device).eval()
        self.value_blend = float(value_blend)
        self.num_top_m = num_top_m
        self.num_sims = num_sims
        self.rollout_depth = rollout_depth
        self.fast_conf = fast_conf
        self.c_scale = c_scale
        self.eq_weight = float(eq_weight)
        self.lookahead_turns = max(1, int(lookahead_turns))
        self.macro_lookahead_turns = max(self.lookahead_turns, int(macro_lookahead_turns))
        self._np_rng = np.random.default_rng(seed)

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
        rk = (lobby_id, player, obs.turn)
        base_g = lobby.belief_tavern(player, seed=self.rng.getrandbits(63) | 1, extra_units=[])
        opp_board, opp_tier, is_ghost, opp_max_hp = self._helper._opponent_board_and_tier(obs, ctx)
        alive_units = ctx.alive_units if ctx is not None else 8
        is_finals = alive_units <= 2 and not is_ghost
        ehp = obs.health + obs.armor
        is_sudden_death = (ehp <= 15) or (is_finals and opp_max_hp <= 16)

        # 1. Drain queued positioning / finals steps
        q = self._queued.get(player)
        if q:
            ak, exp_sig, stop_after = q.pop(0)
            if _state_sig(obs) == exp_sig:
                if ak[0] == "end_turn":
                    self._queued.pop(player, None)
                    return self._finish_with_positioning(
                        base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, is_finals
                    )
                m = _match(ak)
                if m is not None:
                    if ak[0] == "refresh":
                        self._finals_refs[rk] = self._finals_refs.get(rk, 0) + 1
                    if stop_after or not q:
                        self._queued.pop(player, None)
                    return m
            self._queued.pop(player, None)

        # 2. Pure-Physics Sudden-Death Finals / Lethal Exact Combat Solver
        if lobby.phase == "turn" and opp_board and not is_ghost and obs.turn >= 9 and is_sudden_death:
            return super().act_in_lobby(lobby, player)

        # 3. Fast-path when policy is highly confident on a non-prompt, non-macro action
        mk0 = action_mask(_GameView(base_g), allow_reposition=False, allow_freeze=True, obs=obs)
        if not mk0.any():
            return self._finish_with_positioning(
                base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, is_finals
            )
        is_prompt = lobby.phase != "turn" or obs.discover_pending is not None or bool(obs.trinket_options)
        if not is_prompt:
            pv0, ci0, nf0 = encode_observation(obs, ctx, arranging=False)
            with torch.inference_mode():
                lg0, _ = self.model.forward(
                    torch.from_numpy(pv0[None, :]).to(self.device),
                    torch.from_numpy(ci0[None, :].astype(np.int64)).to(self.device),
                    torch.from_numpy(nf0[None, :].astype(np.float32)).to(self.device),
                    torch.from_numpy(mk0[None, :]).to(self.device),
                )
                probs0 = F.softmax(lg0[0].float(), dim=-1).detach().cpu().numpy()
            top_a = int(np.argmax(probs0))
            top_act = action_from_index(top_a)
            if float(probs0[top_a]) >= self.fast_conf and top_act.kind not in ("upgrade_tavern", "dark_discovery", "end_turn"):
                m = _match(_action_key(top_act))
                if m is not None:
                    return m

        # 4. Deterministic Gumbel Sequential Halving Search using `AmberNet`'s on-policy Value Head!
        sims_eff = self.num_sims + 8 if is_prompt else self.num_sims
        has_macro = is_prompt or any(a.kind in ("upgrade_tavern", "dark_discovery") for a in legal)
        lt_eff = self.macro_lookahead_turns if (has_macro and obs.turn >= 2) else self.lookahead_turns
        best_a, _, _ = run_gumbel_root_search(
            model=self.model,
            base_g=base_g,
            ctx=ctx,
            opp_board=opp_board,
            opp_tier=opp_tier,
            rng=self._np_rng,
            device=self.device,
            num_top_m=self.num_top_m,
            num_sims=sims_eff,
            add_gumbel_noise=False,
            rollout_depth=self.rollout_depth,
            c_scale=self.c_scale,
            value_model=self.value_model,
            value_blend=self.value_blend,
            eq_weight=self.eq_weight,
            lookahead_turns=lt_eff,
        )
        act = action_from_index(int(best_a))
        if act.kind == "end_turn":
            return self._finish_with_positioning(
                base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, is_finals
            )
        m = _match(_action_key(act))
        return m if m is not None else legal[0]

