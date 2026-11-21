"""PolicyGuidedBeamBot: Zero-Hardcoding AlphaZero Policy-Pruned Full-Turn Beam Search!

Combines the best of both worlds with ZERO hardcoded card/comp/trinket/hero rules:
  1. Multi-step combinatorial intra-turn planning (discovers 4-10 step sell/buy/play/triple/spell
     combos that 1-step root Gumbel search misses!).
  2. Strict Anti-Goodhart Top-K Policy Expansion + KL Prior Regularization (`E[V_onpolicy] + beta * mean_log_pi`)
     using our calibrated on-policy `AmberNet` (`val_ce = 1.609`), keeping every candidate strictly inside
     the learned neural policy manifold!
  3. Exact CRN-paired stochastic boundary rollouts (`refresh`, `discover`, `trinket`, `dark_discovery`)
     to the symmetric Bellman boundary (`EOT -> Fast Keyword Order -> Combat t -> Start-of-Turn t+1`,
     or 2 full turns on irreversible macro/prompt decisions!).
  4. Pure-Physics Exact EOT Positioning + Sudden-Death Finals Combat Solver (inherited from `FinalsPhysicsRLBot`).
"""

from __future__ import annotations

import math
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

_RL_DIR = str(Path(__file__).resolve().parents[3] / "rl")
if _RL_DIR not in sys.path:
    sys.path.insert(0, _RL_DIR)

import numpy as np
import seaglass
import torch
import torch.nn.functional as F

from seaglass_rl.actions import ACTION_TO_INDEX, action_from_index, action_mask
from seaglass_rl.features import encode_observation

from .gumbel_az import _fast_keyword_order_game
from .policy_bot import _GameView
from .rl_physics_bot import FinalsPhysicsRLBot
from .zero_bot import _action_key, _state_sig

_STOCHASTIC_KINDS = frozenset(
    {"refresh", "dark_discovery", "choose_discover", "choose_trinket", "hero_power", "hero_power_2"}
)


@dataclass
class _BeamNode:
    game: seaglass.TavernGame
    plan: List[int]
    sum_logp: float
    done_turn: bool
    stochastic_leaf: bool
    has_macro: bool


class PolicyGuidedBeamBot(FinalsPhysicsRLBot):
    """AlphaZero-style Policy-Pruned Multi-Step Intra-Turn Beam Search (ZERO hardcoding!)."""

    def __init__(
        self,
        policy_ckpt: str,
        beam_width: int = 6,
        k_expand: int = 3,
        max_depth: int = 9,
        min_prob: float = 0.04,
        n_crn: int = 3,
        rollout_depth: int = 12,
        kl_prior: float = 0.18,
        macro_lookahead_turns: int = 2,
        use_cuda: bool = False,
        seed: int = 2029,
        **kwargs,
    ) -> None:
        super().__init__(policy_ckpt=policy_ckpt, seed=seed, **kwargs)
        self.device = torch.device("cuda" if (use_cuda and torch.cuda.is_available()) else "cpu")
        self.model = self.model.to(self.device).eval()
        self.beam_width = int(beam_width)
        self.k_expand = int(k_expand)
        self.max_depth = int(max_depth)
        self.min_prob = float(min_prob)
        self.n_crn = max(1, int(n_crn))
        self.rollout_depth = int(rollout_depth)
        self.kl_prior = float(kl_prior)
        self.macro_lookahead_turns = max(1, int(macro_lookahead_turns))
        self._np_rng = np.random.default_rng(seed)

    def _batched_policy_value(
        self,
        games: List[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
    ) -> Tuple[np.ndarray, np.ndarray, List[np.ndarray]]:
        pvs, cis, nfs, mks = [], [], [], []
        for g in games:
            o = g.observe()
            mk = action_mask(_GameView(g), allow_reposition=False, allow_freeze=True, obs=o)
            pv, ci, nf = encode_observation(o, ctx, arranging=False)
            pvs.append(pv)
            cis.append(ci)
            nfs.append(nf)
            mks.append(mk)
        with torch.inference_mode():
            with torch.amp.autocast("cuda", dtype=torch.float16, enabled=self.device.type == "cuda"):
                lg, val = self.model.forward(
                    torch.from_numpy(np.stack(pvs, axis=0).astype(np.float32)).to(self.device),
                    torch.from_numpy(np.stack(cis, axis=0).astype(np.int64)).to(self.device),
                    torch.from_numpy(np.stack(nfs, axis=0).astype(np.float32)).to(self.device),
                    torch.from_numpy(np.stack(mks, axis=0).astype(np.bool_)).to(self.device),
                )
            lg_f = lg.float()
            probs = F.softmax(lg_f, dim=-1).detach().cpu().numpy()
            vals = val.float().detach().cpu().numpy()
        return probs, vals, mks

    def _search_best_plan(
        self,
        base_g: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        is_prompt: bool,
    ) -> List[int]:
        end_idx = ACTION_TO_INDEX[("end_turn", None, None)]
        upg_idx = ACTION_TO_INDEX.get(("upgrade_tavern", None, None), -1)
        dd_idx = ACTION_TO_INDEX.get(("dark_discovery", None, None), -1)

        # Prompt states (`hero`, `trinket`, `discover`) expand directly at root -> CRN rollouts!
        o0 = base_g.observe()
        is_hero = bool(o0.hero_options) or o0.turn <= 0

        frontier: List[_BeamNode] = [
            _BeamNode(
                game=base_g.copy(),
                plan=[],
                sum_logp=0.0,
                done_turn=False,
                stochastic_leaf=False,
                has_macro=is_prompt,
            )
        ]
        completed: List[_BeamNode] = []

        max_d = 1 if is_prompt else self.max_depth
        for depth in range(max_d):
            if not frontier:
                break
            probs_arr, _, mks = self._batched_policy_value([n.game for n in frontier], ctx)
            next_cands: List[_BeamNode] = []
            seen_sigs: Dict[Tuple[Any, ...], int] = {}

            for ni, node in enumerate(frontier):
                mk = mks[ni]
                leg = np.flatnonzero(mk)
                if len(leg) == 0:
                    completed.append(
                        _BeamNode(node.game, node.plan, node.sum_logp, True, False, node.has_macro)
                    )
                    continue
                pr = probs_arr[ni]
                order = leg[np.argsort(pr[leg])[::-1]]
                chosen: List[int] = []
                k_lim = len(leg) if is_prompt else self.k_expand
                for a_i in order[:k_lim]:
                    if not chosen or float(pr[a_i]) >= self.min_prob or is_prompt:
                        chosen.append(int(a_i))
                # Always allow macro upgrade/dark_discovery at root if competitive (top-6)
                if depth == 0 and not is_prompt:
                    top6 = {int(x) for x in order[: min(6, len(order))]}
                    for ma in (upg_idx, dd_idx):
                        if ma >= 0 and ma in top6 and ma not in chosen and float(pr[ma]) >= 0.02:
                            chosen.append(ma)

                for a_i in chosen:
                    act = action_from_index(a_i)
                    lp = math.log(max(1e-6, float(pr[a_i])))
                    new_plan = node.plan + [a_i]
                    new_lp = node.sum_logp + lp
                    new_macro = node.has_macro or (act.kind in ("upgrade_tavern", "dark_discovery", "choose_trinket", "choose_discover"))

                    if act.kind == "end_turn":
                        completed.append(
                            _BeamNode(node.game.copy(), new_plan, new_lp, True, False, new_macro)
                        )
                        continue

                    if act.kind in _STOCHASTIC_KINDS or is_prompt:
                        # Stop deterministic beam at stochastic boundary; evaluate via CRN fresh resamples!
                        completed.append(
                            _BeamNode(node.game.copy(), new_plan, new_lp, False, True, new_macro)
                        )
                        continue

                    sg = node.game.copy()
                    if not sg.is_legal(act):
                        continue
                    ended = sg.step(act)
                    o_next = sg.observe()
                    # If deterministic play triggered a discover prompt, treat as stochastic boundary
                    if ended or o_next.discover_pending is not None or bool(o_next.trinket_options):
                        completed.append(
                            _BeamNode(sg, new_plan, new_lp, bool(ended), not ended, new_macro)
                        )
                        continue

                    sig = _state_sig(o_next)
                    prev_idx = seen_sigs.get(sig)
                    if prev_idx is not None:
                        if new_lp > next_cands[prev_idx].sum_logp:
                            next_cands[prev_idx] = _BeamNode(sg, new_plan, new_lp, False, False, new_macro)
                    else:
                        seen_sigs[sig] = len(next_cands)
                        next_cands.append(_BeamNode(sg, new_plan, new_lp, False, False, new_macro))

            if not next_cands:
                break
            # Prune frontier to top `beam_width` using fast policy-prior + on-policy value!
            if len(next_cands) > self.beam_width:
                _, vals_next, _ = self._batched_policy_value([n.game for n in next_cands], ctx)
                scored = sorted(
                    range(len(next_cands)),
                    key=lambda idx: float(vals_next[idx]) + self.kl_prior * (next_cands[idx].sum_logp / max(1, len(next_cands[idx].plan))),
                    reverse=True,
                )
                frontier = [next_cands[i] for i in scored[: self.beam_width]]
            else:
                frontier = next_cands

        # Any surviving open frontier nodes become unfinished leaves rolled out to EOT
        for fn in frontier:
            completed.append(fn)

        if not completed:
            return [end_idx]

        # Keep ALL unique macro plans + top remaining plans by prior (no length penalty against 1-step upgrade!)
        macro_cands = [n for n in completed if n.has_macro]
        other_cands = [n for n in completed if not n.has_macro]
        macro_cands.sort(key=lambda n: n.sum_logp, reverse=True)
        other_cands.sort(key=lambda n: n.sum_logp, reverse=True)
        lim = max(self.beam_width * 2, 10)
        cands = macro_cands[: max(4, lim // 2)] + other_cands[: max(4, lim - min(len(macro_cands), max(4, lim // 2)))]
        cands = cands[:lim]

        # Build alive lobby opponent pool for Turn t+1 Monte Carlo combat when 2-turn horizon triggers
        alive_pool: List[Tuple[List[seaglass.Unit], int]] = []
        if ctx is not None and getattr(ctx, "opponents", None):
            for o_info in ctx.opponents:
                sb = getattr(o_info, "seen_board", None)
                if getattr(o_info, "alive", True) and sb:
                    alive_pool.append((list(sb), int(getattr(o_info, "tavern_tier", opp_tier))))
        if not alive_pool and opp_board:
            alive_pool.append((opp_board, opp_tier))

        use_2turn = (
            (not is_hero)
            and o0.turn >= 2
            and self.macro_lookahead_turns >= 2
            and any(c.has_macro for c in cands)
        )
        n_turns_eff = 2 if use_2turn else 1

        # Expand CRN-paired fresh RNG particles for EVERY candidate plan!
        crn_seeds = [int(self._np_rng.integers(1, 2**62)) | 1 for _ in range(self.n_crn)]
        sims: List[seaglass.TavernGame] = []
        sim_ci: List[int] = []
        sim_seed: List[int] = []
        turn_done: List[bool] = []
        dead_mask: List[bool] = []

        for ci_idx, cand in enumerate(cands):
            for s in crn_seeds:
                sg = cand.game.copy()
                sg.reseed(s)
                ended = cand.done_turn
                if cand.stochastic_leaf and cand.plan:
                    last_act = action_from_index(cand.plan[-1])
                    if sg.is_legal(last_act):
                        ended = bool(sg.step(last_act))
                sims.append(sg)
                sim_ci.append(ci_idx)
                sim_seed.append(s)
                turn_done.append(bool(ended or is_hero))
                dead_mask.append(False)

        # Roll every particle cleanly to canonical Bellman boundary (`Start-of-Turn t+n_turns_eff`)
        for turn_step in range(n_turns_eff):
            if turn_step == 0:
                live_idx = [i for i, td in enumerate(turn_done) if not td]
            else:
                live_idx = [i for i in range(len(sims)) if (not dead_mask[i]) and (not is_hero)]

            for _ in range(self.rollout_depth):
                if not live_idx:
                    break
                pvs, cis, nfs, mks, keep = [], [], [], [], []
                for idx in live_idx:
                    g = sims[idx]
                    o = g.observe()
                    mk = action_mask(_GameView(g), allow_reposition=False, allow_freeze=True, obs=o)
                    if not mk.any():
                        continue
                    pv, ci, nf = encode_observation(o, ctx, arranging=False)
                    pvs.append(pv)
                    cis.append(ci)
                    nfs.append(nf)
                    mks.append(mk)
                    keep.append(idx)
                if not keep:
                    break
                with torch.inference_mode():
                    with torch.amp.autocast("cuda", dtype=torch.float16, enabled=self.device.type == "cuda"):
                        lg, _ = self.model.forward(
                            torch.from_numpy(np.stack(pvs, axis=0).astype(np.float32)).to(self.device),
                            torch.from_numpy(np.stack(cis, axis=0).astype(np.int64)).to(self.device),
                            torch.from_numpy(np.stack(nfs, axis=0).astype(np.float32)).to(self.device),
                            torch.from_numpy(np.stack(mks, axis=0).astype(np.bool_)).to(self.device),
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

            for idx, g in enumerate(sims):
                if dead_mask[idx] or is_hero:
                    continue
                s = sim_seed[idx]
                _fast_keyword_order_game(g)
                if g.is_legal(seaglass.TavernAction.end_turn()):
                    g.end_turn()
                if turn_step == 0:
                    ob, ot = opp_board, opp_tier
                else:
                    if alive_pool:
                        ob, ot = alive_pool[(s ^ (turn_step * 0x517C)) % len(alive_pool)]
                    else:
                        ob, ot = [], opp_tier
                if ob:
                    g.resolve_combat_against(
                        ob, opponent_tier=ot, seed=(s ^ (0x9E37 + turn_step * 0x1337)) | 1
                    )
                if g.observe().health <= 0:
                    dead_mask[idx] = True
                else:
                    g.reseed((s ^ (0xA5A5 + turn_step * 0x2713)) | 1)
                    g.start_turn()

        # Single batched leaf value pass at Start-of-Turn `t + n_turns_eff`
        leaf_vals = np.full(len(sims), -1.0, dtype=np.float32)
        eval_idx = [i for i, d in enumerate(dead_mask) if not d]
        if eval_idx:
            pvs, cis, nfs = [], [], []
            for idx in eval_idx:
                pv, ci, nf = encode_observation(sims[idx].observe(), ctx, arranging=False)
                pvs.append(pv)
                cis.append(ci)
                nfs.append(nf)
            with torch.inference_mode():
                with torch.amp.autocast("cuda", dtype=torch.float16, enabled=self.device.type == "cuda"):
                    _, vals_t = self.model.forward(
                        torch.from_numpy(np.stack(pvs, axis=0).astype(np.float32)).to(self.device),
                        torch.from_numpy(np.stack(cis, axis=0).astype(np.int64)).to(self.device),
                        torch.from_numpy(np.stack(nfs, axis=0).astype(np.float32)).to(self.device),
                        None,
                    )
                vs = vals_t.float().detach().cpu().numpy()
            for idx, v in zip(eval_idx, vs):
                leaf_vals[idx] = float(v)

        q_sum = np.zeros(len(cands), dtype=np.float64)
        q_cnt = np.zeros(len(cands), dtype=np.float64)
        for ci_idx, lv in zip(sim_ci, leaf_vals):
            q_sum[ci_idx] += float(lv)
            q_cnt[ci_idx] += 1.0
        q_mean = q_sum / np.maximum(1.0, q_cnt)

        best_ci = max(
            range(len(cands)),
            key=lambda i: float(q_mean[i]) + self.kl_prior * (cands[i].sum_logp / max(1, len(cands[i].plan))),
        )
        return cands[best_ci].plan if cands[best_ci].plan else [end_idx]

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

        # 1. Drain queued deterministic multi-step plan (or positioning / finals steps)
        q = self._queued.get(player)
        if q:
            ak, exp_sig, stop_after = q.pop(0)
            if exp_sig is None or _state_sig(obs) == exp_sig:
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

        # 3. Policy-Guided Multi-Step Intra-Turn Beam Search!
        is_prompt = lobby.phase != "turn" or obs.discover_pending is not None or bool(obs.trinket_options)
        plan = self._search_best_plan(base_g, ctx, opp_board, opp_tier, is_prompt=is_prompt)

        # Queue deterministic continuation steps cleanly (re-planning after any stochastic step!)
        first_act = action_from_index(int(plan[0]))
        if len(plan) > 1 and not is_prompt and first_act.kind not in _STOCHASTIC_KINDS:
            sim_q = base_g.copy()
            if sim_q.is_legal(first_act) and not sim_q.step(first_act):
                q_items: List[Tuple[Tuple[str, Any, Any], Any, bool]] = []
                for step_idx in plan[1:]:
                    sa = action_from_index(int(step_idx))
                    sig_now = _state_sig(sim_q.observe())
                    is_stoch = sa.kind in _STOCHASTIC_KINDS or sa.kind == "end_turn"
                    q_items.append((_action_key(sa), sig_now, is_stoch))
                    if is_stoch or not sim_q.is_legal(sa):
                        break
                    if sim_q.step(sa):
                        break
                if q_items:
                    self._queued[player] = q_items

        if first_act.kind == "end_turn":
            return self._finish_with_positioning(
                base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, is_finals
            )
        m = _match(_action_key(first_act))
        return m if m is not None else legal[0]
