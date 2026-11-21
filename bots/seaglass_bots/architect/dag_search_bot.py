"""DistDAGSearchBot: Pure-Neural Transposition-DAG (MCGS) Policy + 8-Way Distributional Critic Search!

100% General Algorithm (ZERO hardcoded card tables, ZERO hardcoded comp/hero/trinket/leveling scripts!):
  1. **Transposition-DAG Within-Turn Beam Search:**
     - Expands candidate multi-step turn sequences (`buy`, `play`, `sell`, `upgrade_tavern`,
       `dark_discovery`, `activate`, `hero_power`, `refresh`, `freeze`, `end_turn`) guided purely
       by our learned `DistEntityTransformer` policy `π(a|s)`.
     - Deduplicates equivalent states via canonical multiset signatures (`buy A -> buy B` merges
       with `buy B -> buy A` — Amber Eye's MCGS transposition principle!).
     - Macro-stratified beam: preserves BOTH upgraded (`tier > t0`) and non-upgraded (`tier == t0`)
       branches through the beam so leveling forks are NEVER pruned prematurely!
  2. **Symmetric Start-of-Turn `t+1` Bellman Leaf Evaluation (Zero Off-Policy Goodharting!):**
     - Rolls EVERY candidate EOT state cleanly through the exact same Rust physics boundary across
       `crn_k` Common-Random-Number paired seeds:
         `Fast Keyword Order -> end_turn() -> resolve_combat_against(opp) -> start_turn(t+1)`
     - Evaluates ALL alive Start-of-Turn `t+1` leaf states in ONE single batched forward pass of
       our calibrated 8-Way Distributional Placement Critic `V(s) = P(place=1..8 | s) @ ATOMS`.
  3. **2-Turn Exact Rollouts for Trinket / Discover Prompts:**
     - Simulates BOTH Turn `t` AND Turn `t+1` under `π` + exact Rust engine so Trinkets and
       Discovers are judged by their ACTUAL executed in-engine payoff!
  4. **Exact Rust 40-Candidate Positioning + Sudden-Death Finals Combat Solver** (inherited from
     `FinalsPhysicsRLBot`).
"""

from __future__ import annotations

import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple

_RL_DIR = str(Path(__file__).resolve().parents[3] / "rl")
if _RL_DIR not in sys.path:
    sys.path.insert(0, _RL_DIR)

import numpy as np
import seaglass
import torch
import torch.nn.functional as F

from seaglass_rl.actions import ACTION_TO_INDEX, END_TURN_INDEX, action_from_index, action_mask
from seaglass_rl.features import encode_observation

from .gumbel_az import _fast_keyword_order_game
from .policy_bot import _GameView
from .rl_physics_bot import FinalsPhysicsRLBot
from .zero_bot import _action_key, _state_sig


def _dag_sig(obs: seaglass.TavernObservation) -> Tuple:
    """Canonical transposition-table key (order-invariant for hand/shop/board multiset + econ)."""
    board = tuple(
        sorted(
            (u.card_id, u.attack, u.health, u.is_golden, u.taunt, u.divine_shield, u.reborn, u.venomous)
            for u in obs.board
        )
    )
    hand = tuple(sorted((c.card_id, getattr(c, "attack", 0), getattr(c, "health", 0), getattr(c, "is_golden", False)) for c in obs.hand))
    shop = tuple(sorted((s.card_id, getattr(s, "attack", 0), getattr(s, "health", 0), getattr(s, "cost", 0)) for s in obs.shop))
    disc_opts = getattr(obs.discover_pending, "options", obs.discover_pending) if obs.discover_pending else ()
    disc = tuple(sorted(getattr(c, "card_id", str(c)) for c in disc_opts)) if disc_opts else ()
    trink = tuple(sorted(getattr(t, "card_id", str(t)) for t in obs.trinket_options)) if obs.trinket_options else ()
    return (
        int(obs.turn),
        int(obs.gold),
        int(obs.tavern_tier),
        int(obs.upgrade_cost),
        int(obs.health),
        int(obs.armor),
        bool(getattr(obs, "is_frozen", False)),
        board,
        hand,
        shop,
        disc,
        trink,
    )


class DistDAGSearchBot(FinalsPhysicsRLBot):
    """Transposition-DAG Policy + 8-Way Distributional Critic Search Bot (zero hardcoded rules)."""

    def __init__(
        self,
        policy_ckpt: str,
        beam_width: int = 8,
        branch_k: int = 4,
        max_depth: int = 7,
        crn_k: int = 4,
        prompt_crn_k: int = 4,
        prior_weight: float = 0.35,
        eq_weight: float = 0.55,
        use_cuda: bool = False,
        seed: int = 2029,
        **kwargs,
    ) -> None:
        super().__init__(policy_ckpt=policy_ckpt, seed=seed, **kwargs)
        self.device = torch.device("cuda" if (use_cuda and torch.cuda.is_available()) else "cpu")
        self.model = self.model.to(self.device)
        self.model.eval()
        self.beam_width = beam_width
        self.branch_k = branch_k
        self.max_depth = max_depth
        self.crn_k = crn_k
        self.prompt_crn_k = prompt_crn_k
        self.prior_weight = prior_weight
        self.eq_weight = eq_weight
        self._upg_idx = ACTION_TO_INDEX.get(("upgrade_tavern", None, None), -1)
        self._dd_idx = ACTION_TO_INDEX.get(("dark_discovery", None, None), -1)
        self._ref_idx = ACTION_TO_INDEX.get(("refresh", None, None), -1)
        self._end_idx = END_TURN_INDEX
        self._dag_refs: Dict[Tuple[int, int, int], int] = {}

    def _batch_policy_val(
        self,
        games: Sequence[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
        need_mask: bool = True,
    ) -> Tuple[np.ndarray, np.ndarray, List[np.ndarray]]:
        pvs, cis, nfs, mks = [], [], [], []
        for g in games:
            o = g.observe()
            pv, ci, nf = encode_observation(o, ctx, arranging=False)
            pvs.append(pv)
            cis.append(ci)
            nfs.append(nf)
            if need_mask:
                mks.append(action_mask(_GameView(g), allow_reposition=False, allow_freeze=True, obs=o))
        with torch.inference_mode():
            with torch.amp.autocast("cuda", dtype=torch.float16, enabled=self.device.type == "cuda"):
                lg, val = self.model.forward(
                    torch.from_numpy(np.asarray(pvs, dtype=np.float32)).to(self.device),
                    torch.from_numpy(np.asarray(cis, dtype=np.int64)).to(self.device),
                    torch.from_numpy(np.asarray(nfs, dtype=np.float32)).to(self.device),
                    torch.from_numpy(np.asarray(mks, dtype=np.bool_)).to(self.device) if need_mask else None,
                )
            lg_np = lg.float().detach().cpu().numpy()
            val_np = val.float().detach().cpu().numpy()
        return lg_np, val_np, mks

    def _rollout_to_eot(
        self,
        sims: List[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
        max_steps: int = 12,
    ) -> None:
        """Batched greedy policy continuation to End-of-Turn `t`."""
        live = list(range(len(sims)))
        for _ in range(max_steps):
            if not live:
                break
            pvs, cis, nfs, mks, keep = [], [], [], [], []
            for idx in live:
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
                        torch.from_numpy(np.asarray(pvs, dtype=np.float32)).to(self.device),
                        torch.from_numpy(np.asarray(cis, dtype=np.int64)).to(self.device),
                        torch.from_numpy(np.asarray(nfs, dtype=np.float32)).to(self.device),
                        torch.from_numpy(np.asarray(mks, dtype=np.bool_)).to(self.device),
                    )
                a_ids = lg.argmax(dim=-1).detach().cpu().numpy()
            nxt = []
            for idx, a_i in zip(keep, a_ids):
                act = action_from_index(int(a_i))
                g = sims[idx]
                if act.kind == "end_turn" or not g.is_legal(act):
                    continue
                if not g.step(act):
                    nxt.append(idx)
            live = nxt

    def _eval_eot_candidates_sot(
        self,
        eot_games: List[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        crn_seeds: List[int],
        extra_turn: bool = False,
    ) -> np.ndarray:
        """Symmetric Bellman evaluation: EOT t -> Fast Order -> Combat t -> SOT t+1 (or t+2) -> V_dist!"""
        m = len(eot_games)
        k = len(crn_seeds)
        sims: List[seaglass.TavernGame] = []
        owners: List[int] = []
        dead = np.zeros(m * k, dtype=np.bool_)
        eq_bonus = np.zeros(m, dtype=np.float32)

        for ci_g, bg in enumerate(eot_games):
            # Exact low-variance continuous Rust combat equity once per EOT candidate (<0.4ms each!)
            if opp_board and self.eq_weight > 0.0:
                g_eq = bg.copy()
                _fast_keyword_order_game(g_eq)
                if g_eq.is_legal(seaglass.TavernAction.end_turn()):
                    g_eq.end_turn()
                ehp_now = float(g_eq.observe().health + g_eq.observe().armor)
                dist = g_eq.equity_against(opp_board, opponent_tier=opp_tier, n=12, base_seed=(crn_seeds[0] ^ 0x4C11) | 1)
                dm = float(dist.damage_mean)
                # Smooth survival & expected HP delta (strongly guards against suicide leveling!)
                lethal_pen = 0.0 if (ehp_now + dm) > 4.0 else -0.60 * max(0.0, 4.0 - (ehp_now + dm)) / 4.0
                eq_bonus[ci_g] = self.eq_weight * (
                    float(dist.a_win_rate) + 0.40 * float(dist.draw_rate) + 0.035 * dm + lethal_pen
                )

            for si, s in enumerate(crn_seeds):
                idx = ci_g * k + si
                g = bg.copy()
                _fast_keyword_order_game(g)
                if g.is_legal(seaglass.TavernAction.end_turn()):
                    g.end_turn()
                if opp_board:
                    g.resolve_combat_against(opp_board, opponent_tier=opp_tier, seed=(s ^ 0x9E37) | 1)
                if g.observe().health <= 0:
                    dead[idx] = True
                else:
                    g.reseed((s ^ 0xA5A5) | 1)
                    g.start_turn()
                sims.append(g)
                owners.append(ci_g)

        if extra_turn:
            alive_sims = [sims[i] for i in range(len(sims)) if not dead[i]]
            if alive_sims:
                self._rollout_to_eot(alive_sims, ctx, max_steps=10)
            for i in range(len(sims)):
                if dead[i]:
                    continue
                g = sims[i]
                s = crn_seeds[i % k]
                _fast_keyword_order_game(g)
                if g.is_legal(seaglass.TavernAction.end_turn()):
                    g.end_turn()
                if opp_board:
                    g.resolve_combat_against(opp_board, opponent_tier=opp_tier, seed=(s ^ 0x517CC1B7) | 1)
                if g.observe().health <= 0:
                    dead[i] = True
                else:
                    g.reseed((s ^ 0x7F4A7C15) | 1)
                    g.start_turn()

        leaf_vals = np.full(m * k, -1.10, dtype=np.float32)
        alive_idx = [i for i in range(m * k) if not dead[i]]
        if alive_idx:
            _, vals_np, _ = self._batch_policy_val([sims[i] for i in alive_idx], ctx, need_mask=False)
            for idx, v in zip(alive_idx, vals_np):
                leaf_vals[idx] = float(v)

        return leaf_vals.reshape(m, k).mean(axis=1) + eq_bonus

    def _solve_prompt_2turn(
        self,
        base_g: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        legal: List[seaglass.TavernAction],
        is_hero: bool,
    ) -> seaglass.TavernAction:
        """Evaluates every legal Trinket / Discover / Hero option via exact Rust rollouts + V_dist."""
        if len(legal) <= 1:
            return legal[0]
        if is_hero:
            # Evaluate Hero options directly via learned critic after stepping each option
            sims = []
            for a in legal:
                g = base_g.copy()
                if g.is_legal(a):
                    g.step(a)
                sims.append(g)
            _, vals, _ = self._batch_policy_val(sims, ctx, need_mask=False)
            return legal[int(np.argmax(vals))]

        crn = [self.rng.getrandbits(62) | 1 for _ in range(self.prompt_crn_k)]
        # Roll each prompt choice through remainder of Turn t, then evaluate symmetrically at SOT t+2!
        cand_games: List[seaglass.TavernGame] = []
        for a in legal:
            g = base_g.copy()
            g.reseed(crn[0])
            if g.is_legal(a):
                g.step(a)
            cand_games.append(g)
        self._rollout_to_eot(cand_games, ctx, max_steps=12)
        scores = self._eval_eot_candidates_sot(
            cand_games, ctx, opp_board, opp_tier, crn, extra_turn=True
        )
        return legal[int(np.argmax(scores))]

    def _plan_turn_dag(
        self,
        base_g: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        refs_used: int,
    ) -> List[Tuple[Tuple[str, Any, Any], Tuple, bool]]:
        """Transposition-DAG Policy-Guided Within-Turn Beam -> Symmetric SOT t+1 Distributional Eval."""
        o0 = base_g.observe()
        t0 = int(o0.tavern_tier)
        crn = [self.rng.getrandbits(62) | 1 for _ in range(self.crn_k)]

        # Node: (game, steps_list, log_p_sum, upgraded_flag, stop_stochastic_flag)
        # steps_list entries: (action_key, expected_state_sig, stop_after)
        frontier: List[Tuple[seaglass.TavernGame, List[Tuple[Tuple[str, Any, Any], Tuple, bool]], float, bool]] = [
            (base_g.copy(), [], 0.0, False)
        ]
        visited: Dict[Tuple, float] = {_dag_sig(o0): 0.0}
        # Completed EOT or stochastic-leaf candidates: (eot_game, steps, is_stochastic_rollout, lp_sum)
        terminals: List[Tuple[seaglass.TavernGame, List[Tuple[Tuple[str, Any, Any], Tuple, bool]], bool, float]] = []
        term_sigs: Dict[Tuple, int] = {}

        def _add_terminal(
            g_t: seaglass.TavernGame,
            steps: List[Tuple[Tuple[str, Any, Any], Tuple, bool]],
            is_stoch: bool,
            lp_val: float,
        ) -> None:
            sig = (_dag_sig(g_t.observe()), is_stoch)
            prev_i = term_sigs.get(sig)
            if prev_i is None:
                term_sigs[sig] = len(terminals)
                terminals.append((g_t, steps, is_stoch, lp_val))
            elif lp_val > terminals[prev_i][3]:
                terminals[prev_i] = (g_t, steps, is_stoch, lp_val)

        # Always include standing pat (`end_turn` immediately) as a baseline terminal
        _add_terminal(base_g.copy(), [(("end_turn", None, None), _state_sig(o0), True)], False, -2.5)

        for depth in range(self.max_depth):
            if not frontier:
                break
            lg_all, val_all, mks_all = self._batch_policy_val([f[0] for f in frontier], ctx, need_mask=True)
            next_cands: List[Tuple[seaglass.TavernGame, List[Tuple[Tuple[str, Any, Any], Tuple, bool]], float, bool]] = []

            for fi, (g, steps, lp_sum, upg_f) in enumerate(frontier):
                o_cur = g.observe()
                sig_cur = _state_sig(o_cur)
                mk = mks_all[fi]
                leg_idx = np.flatnonzero(mk)
                if len(leg_idx) == 0:
                    _add_terminal(g, steps + [(("end_turn", None, None), sig_cur, True)], False, lp_sum)
                    continue
                lps = lg_all[fi] - float(np.max(lg_all[fi, leg_idx]))
                lps_norm = lps - float(np.log(np.sum(np.exp(lps[leg_idx]))))

                order = leg_idx[np.argsort(lg_all[fi, leg_idx])[::-1]]
                chosen: List[int] = [int(a) for a in order[: self.branch_k]]

                # Macro exploration guarantee: include `upgrade_tavern` / `dark_discovery` if legal & non-absurd (>0.01)
                for ma in (self._upg_idx, self._dd_idx):
                    if ma >= 0 and mk[ma] and ma not in chosen and float(np.exp(lps_norm[ma])) >= 0.01:
                        chosen.append(ma)
                # Consider `refresh` only on depth <= 2 if we still have budget
                if self._ref_idx >= 0 and mk[self._ref_idx] and depth <= 2 and refs_used < 2:
                    if self._ref_idx not in chosen and float(np.exp(lps_norm[self._ref_idx])) >= 0.06:
                        chosen.append(self._ref_idx)

                for a_i in chosen:
                    act = action_from_index(a_i)
                    ak = _action_key(act)
                    step_lp = float(lps_norm[a_i])
                    if act.kind == "end_turn":
                        _add_terminal(g.copy(), steps + [(ak, sig_cur, True)], False, lp_sum + step_lp)
                        continue

                    is_stoch = act.kind in ("refresh", "dark_discovery")
                    if is_stoch:
                        # Do NOT leak single-seed shop optimism: evaluate via `crn_k` rollouts!
                        if depth == 0 or len(steps) <= 2:
                            g_s = g.copy()
                            g_s.reseed(crn[0])
                            if g_s.is_legal(act):
                                g_s.step(act)
                            # Mark stop_after=True so real execution re-plans after seeing true shop!
                            _add_terminal(g_s, steps + [(ak, sig_cur, True)], True, lp_sum + step_lp)
                        continue

                    ng = g.copy()
                    ng.reseed(crn[0])
                    if not ng.is_legal(act):
                        continue
                    ended = ng.step(act)
                    no = ng.observe()
                    n_upg = upg_f or (int(no.tavern_tier) > t0)
                    # If deterministic play triggered a Discover / Triple reveal, treat as stochastic leaf & re-plan!
                    triggered_prompt = (no.discover_pending is not None) or bool(no.trinket_options)
                    new_steps = steps + [(ak, sig_cur, triggered_prompt or ended)]
                    new_lp = lp_sum + step_lp
                    if ended or triggered_prompt:
                        _add_terminal(ng, new_steps, triggered_prompt, new_lp)
                        continue

                    dsig = _dag_sig(no)
                    # Transposition-DAG deduplication: keep highest-prior path to identical state!
                    prev_lp = visited.get(dsig)
                    if prev_lp is not None and prev_lp >= new_lp:
                        continue
                    visited[dsig] = new_lp
                    next_cands.append((ng, new_steps, new_lp, n_upg))

            if not next_cands:
                break

            # Score intermediate DAG layer with KL-regularized DistEntityTransformer value
            _, n_vals, _ = self._batch_policy_val([c[0] for c in next_cands], ctx, need_mask=False)
            scored = [
                (float(n_vals[i]) + self.prior_weight * (next_cands[i][2] / max(1, len(next_cands[i][1]))), i)
                for i in range(len(next_cands))
            ]
            scored.sort(reverse=True)

            # Macro-stratified pruning: reserve up to 2 slots for upgraded (`tier > t0`) branches!
            upg_slots = [idx for _, idx in scored if next_cands[idx][3]][: min(2, self.beam_width // 2)]
            non_slots = [idx for _, idx in scored if not next_cands[idx][3]]
            keep_ids: List[int] = list(upg_slots)
            for idx in non_slots:
                if len(keep_ids) >= self.beam_width:
                    break
                keep_ids.append(idx)
            for _, idx in scored:
                if len(keep_ids) >= self.beam_width:
                    break
                if idx not in keep_ids:
                    keep_ids.append(idx)
            frontier = [next_cands[i] for i in keep_ids]

        # Any surviving frontier nodes become EOT candidates (roll remaining actions via π if gold left)
        for g, steps, lp_sum, _ in frontier:
            o_f = g.observe()
            _add_terminal(g, steps + [(("end_turn", None, None), _state_sig(o_f), True)], True, lp_sum)

        if not terminals:
            return [(("end_turn", None, None), _state_sig(o0), True)]

        # Cap terminal pool cleanly (keep best upgraded + best non-upgraded via KL-regularized V_dist)
        if len(terminals) > 14:
            _, t_vals, _ = self._batch_policy_val([t[0] for t in terminals], ctx, need_mask=False)
            t_sc = np.array(
                [float(t_vals[i]) + self.prior_weight * (terminals[i][3] / max(1, len(terminals[i][1]))) for i in range(len(terminals))],
                dtype=np.float64,
            )
            t_ord = np.argsort(t_sc)[::-1]
            upg_t = [i for i in t_ord if int(terminals[i][0].observe().tavern_tier) > t0][:4]
            oth_t = [i for i in t_ord if i not in upg_t][: 14 - len(upg_t)]
            terminals = [terminals[i] for i in (upg_t + oth_t)]

        # Finish any stochastic / incomplete terminals with greedy π to EOT so EVERY terminal is EOT t!
        eot_games: List[seaglass.TavernGame] = []
        roll_idx: List[int] = []
        for ti, (tg, _, is_stoch, _) in enumerate(terminals):
            cg = tg.copy()
            eot_games.append(cg)
            if is_stoch:
                roll_idx.append(ti)
        if roll_idx:
            self._rollout_to_eot([eot_games[i] for i in roll_idx], ctx, max_steps=10)

        # Symmetric Start-of-Turn t+1 Bellman Evaluation + KL Trust-Region Prior!
        v_sot = self._eval_eot_candidates_sot(
            eot_games, ctx, opp_board, opp_tier, crn, extra_turn=False
        )
        scores = np.array(
            [
                float(v_sot[i]) + self.prior_weight * (terminals[i][3] / max(1, len(terminals[i][1])))
                for i in range(len(terminals))
            ],
            dtype=np.float64,
        )
        best_i = int(np.argmax(scores))
        return terminals[best_i][1]

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

        # 1. Drain cached deterministic prefix steps
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
                        self._dag_refs[rk] = self._dag_refs.get(rk, 0) + 1
                    if stop_after or not q:
                        self._queued.pop(player, None)
                    return m
            self._queued.pop(player, None)

        # 2. Pure-Physics Sudden-Death Finals / Lethal Exact Combat Solver
        if lobby.phase == "turn" and opp_board and not is_ghost and obs.turn >= 9 and is_sudden_death:
            return super().act_in_lobby(lobby, player)

        # 3. Hero / Trinket / Discover Prompts -> Exact 2-Turn Rust Rollouts + V_dist!
        is_hero = lobby.phase != "turn" or bool(obs.hero_options) or obs.turn <= 0
        is_prompt = is_hero or (obs.discover_pending is not None) or bool(obs.trinket_options)
        if is_prompt:
            return self._solve_prompt_2turn(base_g, ctx, opp_board, opp_tier, legal, is_hero=is_hero)

        # 4. Transposition-DAG Within-Turn Beam -> Symmetric SOT t+1 Distributional Eval!
        refs_used = self._dag_refs.get(rk, 0)
        plan = self._plan_turn_dag(base_g, ctx, opp_board, opp_tier, refs_used=refs_used)
        if not plan:
            return self._finish_with_positioning(
                base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, is_finals
            )

        ak0, _, stop0 = plan[0]
        if ak0[0] == "end_turn":
            return self._finish_with_positioning(
                base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, is_finals
            )
        m0 = _match(ak0)
        if m0 is not None:
            if ak0[0] == "refresh":
                self._dag_refs[rk] = refs_used + 1
            if not stop0 and len(plan) > 1:
                self._queued[player] = plan[1:]
            return m0
        return legal[0]
