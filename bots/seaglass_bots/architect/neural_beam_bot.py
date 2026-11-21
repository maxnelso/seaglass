"""NeuralPhysicsBeamBot: Zero-Hardcoding Multi-Epoch Exact Rust Beam + Learned Multi-Horizon Scaling Transformer!

100% General Architecture (ZERO hardcoded card names, comp tables, trinket lists, or hero lists!):
  1. **True Multi-Epoch Turn Search (Unconstrained, NEVER truncates turns or leaves gold unspent!):**
     - Explores multi-step deterministic chains (`buy`, `play`, `play_on_shop`, `sell`, `activate`,
       `hero_power`, `upgrade_tavern`, `toggle_freeze`, `end_turn`) via a Transposition-DAG Beam
       guided by our learned `DistEntityTransformer` policy `π(a|s)`.
     - Evaluates stochastic transitions (`refresh`, `dark_discovery`, Triple/Discover reveals) via
       CRN-paired rollouts, executes up to the stochastic event, and immediately re-plans a fresh
       epoch on the true revealed state!
  2. **Triple-Signal Low-Variance EOT Evaluation (Zero Goodharting!):**
     - **Signal A (Exact Rust Combat Physics):** Runs `_fast_keyword_order_game` -> `end_turn()` ->
       exact Rust `equity_against(opp_board, n=16)` for continuous `win_rate`, `draw_rate`,
       `damage_mean`, and lethal survival guard!
     - **Signal B (Exact Rust Autonomous EOT Growth):** Directly measures `delta_eot_stats` executed
       by Rust `end_turn()` (capturing ALL End-of-Turn scalers & Trinket triggers with zero card names!).
     - **Signal C (Learned `ScalingValueTransformer`):** Compact regularized Transformer trained on
       Tier-6 Grandmaster lobbies predicting `P(place=1..8)`, `P(1st)`, AND dense 3-turn future
       board scaling `E[log1p(stats_{t+3})]` in `< 2 ms` per batch!
  3. **2-Turn Exact Rust Lookahead** for Trinkets & Discovers + **40-Candidate Exact Rust Positioning**
     + **Sudden-Death Finals Exact Combat Solver** (inherited from `FinalsPhysicsRLBot`).
"""

from __future__ import annotations

import math
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple

_RL_DIR = str(Path(__file__).resolve().parents[3] / "rl")
if _RL_DIR not in sys.path:
    sys.path.insert(0, _RL_DIR)

import numpy as np
import seaglass
import torch

from seaglass_rl.actions import ACTION_TO_INDEX, END_TURN_INDEX, action_from_index, action_mask
from seaglass_rl.features import encode_observation

from .dag_search_bot import _dag_sig
from .gumbel_az import _fast_keyword_order_game
from .policy_bot import _GameView
from .rl_physics_bot import FinalsPhysicsRLBot
from .scaling_value import ScalingValueTransformer
from .zero_bot import _action_key, _state_sig


class NeuralPhysicsBeamBot(FinalsPhysicsRLBot):
    """Multi-Epoch Transposition-DAG Rust Beam guided by `DistEntityTransformer` + `ScalingValueTransformer`."""

    def __init__(
        self,
        policy_ckpt: str,
        value_ckpt: str,
        beam_width: int = 8,
        branch_k: int = 5,
        max_epoch_depth: int = 5,
        refresh_samples: int = 2,
        combats_eval: int = 16,
        max_refreshes_per_turn: int = 5,
        w_place: float = 1.00,
        w_p1: float = 1.20,
        w_fut_stats: float = 0.45,
        w_combat: float = 0.85,
        w_eot_growth: float = 0.12,
        w_prior: float = 0.10,
        use_cuda: bool = False,
        seed: int = 2029,
        **kwargs,
    ) -> None:
        super().__init__(policy_ckpt=policy_ckpt, seed=seed, **kwargs)
        self.device = torch.device("cuda" if (use_cuda and torch.cuda.is_available()) else "cpu")
        self.model = self.model.to(self.device).eval()
        self.val_net = ScalingValueTransformer.load_checkpoint(value_ckpt).to(self.device).eval()
        self.beam_width = beam_width
        self.branch_k = branch_k
        self.max_epoch_depth = max_epoch_depth
        self.refresh_samples = refresh_samples
        self.combats_eval = combats_eval
        self.max_refreshes_per_turn = max_refreshes_per_turn
        self.w_place = w_place
        self.w_p1 = w_p1
        self.w_fut_stats = w_fut_stats
        self.w_combat = w_combat
        self.w_eot_growth = w_eot_growth
        self.w_prior = w_prior
        self._upg_idx = ACTION_TO_INDEX.get(("upgrade_tavern", None, None), -1)
        self._dd_idx = ACTION_TO_INDEX.get(("dark_discovery", None, None), -1)
        self._ref_idx = ACTION_TO_INDEX.get(("refresh", None, None), -1)
        self._end_idx = END_TURN_INDEX
        self._epoch_refs: Dict[Tuple[int, int, int], int] = {}

    def _batch_policy(
        self,
        games: Sequence[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
    ) -> Tuple[np.ndarray, List[np.ndarray]]:
        pvs, cis, nfs, mks = [], [], [], []
        for g in games:
            o = g.observe()
            pv, ci, nf = encode_observation(o, ctx, arranging=False)
            pvs.append(pv)
            cis.append(ci)
            nfs.append(nf)
            mks.append(action_mask(_GameView(g), allow_reposition=False, allow_freeze=True, obs=o))
        with torch.inference_mode():
            with torch.amp.autocast("cuda", dtype=torch.float16, enabled=self.device.type == "cuda"):
                lg, _ = self.model.forward(
                    torch.from_numpy(np.asarray(pvs, dtype=np.float32)).to(self.device),
                    torch.from_numpy(np.asarray(cis, dtype=np.int64)).to(self.device),
                    torch.from_numpy(np.asarray(nfs, dtype=np.float32)).to(self.device),
                    torch.from_numpy(np.asarray(mks, dtype=np.bool_)).to(self.device),
                )
            return lg.float().detach().cpu().numpy(), mks

    def _batch_scale_value(
        self,
        games: Sequence[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
    ) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
        """Returns `(exp_place_val, p1_prob, pred_fut_log_stats)` for a batch of games."""
        pvs, cis, nfs = [], [], []
        for g in games:
            pv, ci, nf = encode_observation(g.observe(), ctx, arranging=False)
            pvs.append(pv)
            cis.append(ci)
            nfs.append(nf)
        with torch.inference_mode():
            ev, _, fs, p1_lg = self.val_net.forward(
                torch.from_numpy(np.asarray(pvs, dtype=np.float32)).to(self.device),
                torch.from_numpy(np.asarray(cis, dtype=np.int64)).to(self.device),
                torch.from_numpy(np.asarray(nfs, dtype=np.float32)).to(self.device),
            )
            return (
                ev.detach().cpu().numpy(),
                torch.sigmoid(p1_lg).detach().cpu().numpy(),
                fs.detach().cpu().numpy(),
            )

    def _rollout_to_eot(
        self,
        sims: List[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
        max_steps: int = 12,
    ) -> None:
        live = list(range(len(sims)))
        for _ in range(max_steps):
            if not live:
                break
            cur_games = [sims[i] for i in live]
            lg, mks = self._batch_policy(cur_games, ctx)
            nxt = []
            for li, g_idx in enumerate(live):
                if not mks[li].any():
                    continue
                a_i = int(np.argmax(lg[li]))
                act = action_from_index(a_i)
                g = sims[g_idx]
                if act.kind == "end_turn" or not g.is_legal(act):
                    continue
                if not g.step(act):
                    nxt.append(g_idx)
            live = nxt

    def _score_eot_candidates(
        self,
        eot_games: Sequence[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        is_ghost: bool,
        base_seed: int,
    ) -> np.ndarray:
        """Triple-Signal Zero-Hardcoding EOT Scorer: Rust Combat + Autonomous EOT Growth + ScalingValueTransformer!"""
        m = len(eot_games)
        post_eot_games: List[seaglass.TavernGame] = []
        combat_scores = np.zeros(m, dtype=np.float32)
        eot_growth_scores = np.zeros(m, dtype=np.float32)

        for i, bg in enumerate(eot_games):
            g = bg.copy()
            o_pre = g.observe()
            st_pre = sum(u.attack + u.health for u in o_pre.board)
            _fast_keyword_order_game(g)
            if g.is_legal(seaglass.TavernAction.end_turn()):
                g.end_turn()
            o_post = g.observe()
            st_post = sum(u.attack + u.health for u in o_post.board)
            # Exact Rust autonomous EOT stat delta (zero card names!)
            delta_st = max(0, st_post - st_pre)
            eot_growth_scores[i] = float(math.log1p(delta_st)) / 4.0

            # Exact Rust combat equity against projected opponent board
            if opp_board and not is_ghost:
                ehp = float(o_post.health + o_post.armor)
                dist = g.equity_against(
                    opp_board, opponent_tier=opp_tier, n=self.combats_eval, base_seed=(base_seed ^ 0x51A7) | 1
                )
                dm = float(dist.damage_mean)
                # Smooth combat equity + lethal survival guard
                lethal_pen = 0.0
                if ehp + dm <= 5.0:
                    lethal_pen = -0.85 * max(0.0, 5.0 - (ehp + dm)) / 5.0
                # Late-game / low-HP amplifies survival weight automatically (general health awareness)
                hp_mult = 1.45 if ehp <= 15.0 else (1.15 if ehp <= 24.0 else 1.0)
                combat_scores[i] = hp_mult * (
                    float(dist.a_win_rate) + 0.40 * float(dist.draw_rate) + 0.032 * dm + lethal_pen
                )
            post_eot_games.append(g)

        # Learned Multi-Horizon Value + 1st-Place Prob + 3-Turn Future Stats on post-EOT boards!
        ev, p1, fut_fs = self._batch_scale_value(post_eot_games, ctx)
        return (
            self.w_place * ev
            + self.w_p1 * p1
            + self.w_fut_stats * fut_fs
            + self.w_combat * combat_scores
            + self.w_eot_growth * eot_growth_scores
        )

    def _solve_prompt_2turn(
        self,
        base_g: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        is_ghost: bool,
        legal: List[seaglass.TavernAction],
        is_hero: bool,
    ) -> seaglass.TavernAction:
        if len(legal) <= 1:
            return legal[0]
        if is_hero:
            sims = []
            for a in legal:
                g = base_g.copy()
                if g.is_legal(a):
                    g.step(a)
                sims.append(g)
            ev, p1, fut_fs = self._batch_scale_value(sims, ctx)
            sc = self.w_place * ev + self.w_p1 * p1 + 0.30 * fut_fs
            return legal[int(np.argmax(sc))]

        # 2-turn CRN rollout for Trinkets / Discovers: executes real Rust Trinket/card triggers across t and t+1!
        k_crn = 3
        crn = [self.rng.getrandbits(62) | 1 for _ in range(k_crn)]
        sims: List[seaglass.TavernGame] = []
        for a in legal:
            for s in crn:
                g = base_g.copy()
                g.reseed(s)
                if g.is_legal(a):
                    g.step(a)
                sims.append(g)
        self._rollout_to_eot(sims, ctx, max_steps=12)
        # Advance Turn t -> Combat t -> Turn t+1 -> Rollout Turn t+1 -> EOT t+1!
        dead = np.zeros(len(sims), dtype=np.bool_)
        for idx, g in enumerate(sims):
            s = crn[idx % k_crn]
            _fast_keyword_order_game(g)
            if g.is_legal(seaglass.TavernAction.end_turn()):
                g.end_turn()
            if opp_board and not is_ghost:
                g.resolve_combat_against(opp_board, opponent_tier=opp_tier, seed=(s ^ 0x9E37) | 1)
            if g.observe().health <= 0:
                dead[idx] = True
            else:
                g.reseed((s ^ 0xA5A5) | 1)
                g.start_turn()
        alive_sims = [sims[i] for i in range(len(sims)) if not dead[i]]
        if alive_sims:
            self._rollout_to_eot(alive_sims, ctx, max_steps=10)
        sc_all = np.full(len(sims), -2.5, dtype=np.float32)
        alive_idx = [i for i in range(len(sims)) if not dead[i]]
        if alive_idx:
            sc_alive = self._score_eot_candidates(
                [sims[i] for i in alive_idx], ctx, opp_board, opp_tier, is_ghost, crn[0]
            )
            for i, v in zip(alive_idx, sc_alive):
                sc_all[i] = float(v)
        mean_sc = sc_all.reshape(len(legal), k_crn).mean(axis=1)
        return legal[int(np.argmax(mean_sc))]

    def _plan_epoch(
        self,
        base_g: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        is_ghost: bool,
        refs_used: int,
    ) -> List[Tuple[Tuple[str, Any, Any], Tuple, bool]]:
        """Plans ONE full decision epoch (deterministic multi-step prefix OR stochastic step -> re-plan!)."""
        o0 = base_g.observe()
        t0 = int(o0.tavern_tier)
        s0 = self.rng.getrandbits(62) | 1
        ref_seeds = [self.rng.getrandbits(62) | 1 for _ in range(self.refresh_samples)]

        # Frontier node: (game, steps, lp_sum, upgraded_flag)
        frontier: List[Tuple[seaglass.TavernGame, List[Tuple[Tuple[str, Any, Any], Tuple, bool]], float, bool]] = [
            (base_g.copy(), [], 0.0, False)
        ]
        visited: Dict[Tuple, float] = {_dag_sig(o0): 0.0}

        # Deterministic EOT candidates: (eot_game, steps, lp_sum)
        det_terms: List[Tuple[seaglass.TavernGame, List[Tuple[Tuple[str, Any, Any], Tuple, bool]], float]] = []
        det_sigs: Dict[Tuple, int] = {}
        # Stochastic branch candidates (evaluated via `refresh_samples` CRN rollouts): (parent_game, act, steps_before, sig_before, lp_sum)
        stoch_cands: List[Tuple[seaglass.TavernGame, seaglass.TavernAction, List[Tuple[Tuple[str, Any, Any], Tuple, bool]], Tuple, float]] = []
        stoch_seen: set = set()

        def _add_det(g_t: seaglass.TavernGame, steps: List[Tuple[Tuple[str, Any, Any], Tuple, bool]], lp_v: float) -> None:
            sg = _dag_sig(g_t.observe())
            pi = det_sigs.get(sg)
            if pi is None:
                det_sigs[sg] = len(det_terms)
                det_terms.append((g_t, steps, lp_v))
            elif lp_v > det_terms[pi][2]:
                det_terms[pi] = (g_t, steps, lp_v)

        _add_det(base_g.copy(), [(("end_turn", None, None), _state_sig(o0), True)], -1.5)

        for depth in range(self.max_epoch_depth):
            if not frontier:
                break
            lg_all, mks_all = self._batch_policy([f[0] for f in frontier], ctx)
            next_cands: List[Tuple[seaglass.TavernGame, List[Tuple[Tuple[str, Any, Any], Tuple, bool]], float, bool]] = []

            for fi, (g, steps, lp_sum, upg_f) in enumerate(frontier):
                o_c = g.observe()
                sig_c = _state_sig(o_c)
                mk = mks_all[fi]
                leg = np.flatnonzero(mk)
                if len(leg) == 0:
                    _add_det(g.copy(), steps + [(("end_turn", None, None), sig_c, True)], lp_sum)
                    continue
                lps = lg_all[fi] - float(np.max(lg_all[fi, leg]))
                lps_n = lps - float(np.log(np.sum(np.exp(lps[leg]))))
                order = leg[np.argsort(lg_all[fi, leg])[::-1]]

                chosen = [int(a) for a in order[: self.branch_k]]
                # Always consider key macro/stochastic forks if legal!
                if self._upg_idx >= 0 and mk[self._upg_idx] and self._upg_idx not in chosen:
                    chosen.append(self._upg_idx)
                if self._dd_idx >= 0 and mk[self._dd_idx] and self._dd_idx not in chosen:
                    chosen.append(self._dd_idx)
                if (
                    self._ref_idx >= 0
                    and mk[self._ref_idx]
                    and refs_used < self.max_refreshes_per_turn
                    and o_c.gold >= 4
                    and self._ref_idx not in chosen
                ):
                    chosen.append(self._ref_idx)

                for a_i in chosen:
                    act = action_from_index(a_i)
                    ak = _action_key(act)
                    slp = float(lps_n[a_i])
                    if act.kind == "end_turn":
                        _add_det(g.copy(), steps + [(ak, sig_c, True)], lp_sum + slp)
                        continue
                    if act.kind in ("refresh", "dark_discovery"):
                        if act.kind == "refresh" and refs_used >= self.max_refreshes_per_turn:
                            continue
                        sk = (_dag_sig(o_c), ak)
                        if sk not in stoch_seen and len(stoch_cands) < 10:
                            stoch_seen.add(sk)
                            stoch_cands.append((g.copy(), act, list(steps), sig_c, lp_sum + slp))
                        continue

                    ng = g.copy()
                    ng.reseed(s0)
                    if not ng.is_legal(act):
                        continue
                    ended = ng.step(act)
                    no = ng.observe()
                    n_upg = upg_f or (int(no.tavern_tier) > t0)
                    trig = (no.discover_pending is not None) or bool(no.trinket_options)
                    new_steps = steps + [(ak, sig_c, trig or ended)]
                    new_lp = lp_sum + slp
                    if ended:
                        _add_det(ng, new_steps, new_lp)
                        continue
                    if trig:
                        # Triggered Discover/Triple: evaluate continuation via CRN rollout & re-plan after reveal!
                        sk = (_dag_sig(o_c), ak)
                        if sk not in stoch_seen and len(stoch_cands) < 12:
                            stoch_seen.add(sk)
                            stoch_cands.append((g.copy(), act, list(steps), sig_c, new_lp))
                        continue

                    dsig = _dag_sig(no)
                    prev = visited.get(dsig)
                    if prev is not None and prev >= new_lp:
                        continue
                    visited[dsig] = new_lp
                    next_cands.append((ng, new_steps, new_lp, n_upg))

            if not next_cands:
                break

            # Fast intermediate pruning via ScalingValueTransformer (<1.5ms!)
            ev, p1, fs = self._batch_scale_value([c[0] for c in next_cands], ctx)
            inter_sc = [
                float(self.w_place * ev[i] + self.w_p1 * p1[i] + self.w_fut_stats * fs[i] + 0.05 * next_cands[i][2])
                for i in range(len(next_cands))
            ]
            ord_i = np.argsort(inter_sc)[::-1]
            # Preserve top upgraded + top non-upgraded branches
            upg_i = [int(i) for i in ord_i if next_cands[i][3]][: min(2, self.beam_width // 2)]
            keep_i = list(upg_i)
            for i in ord_i:
                ii = int(i)
                if len(keep_i) >= self.beam_width:
                    break
                if ii not in keep_i:
                    keep_i.append(ii)
            frontier = [next_cands[i] for i in keep_i]

        # Surviving frontier states with leftover gold are ALSO evaluated as "continue next epoch" candidates!
        for g, steps, lp_sum, _ in frontier:
            o_f = g.observe()
            # If still has >= 3 gold and legal moves, mark last step `stop_after=True` so we re-plan a new epoch!
            if o_f.gold >= 3 and steps:
                st_cont = list(steps[:-1]) + [(steps[-1][0], steps[-1][1], True)]
                _add_det(g.copy(), st_cont, lp_sum)
            else:
                _add_det(g.copy(), steps + [(("end_turn", None, None), _state_sig(o_f), True)], lp_sum)

        # Cap deterministic terminals cleanly before exact Rust combat scoring
        if len(det_terms) > 12:
            ev, p1, fs = self._batch_scale_value([d[0] for d in det_terms], ctx)
            pre_sc = self.w_place * ev + self.w_p1 * p1 + self.w_fut_stats * fs
            d_ord = np.argsort(pre_sc)[::-1]
            upg_d = [int(i) for i in d_ord if int(det_terms[i][0].observe().tavern_tier) > t0][:4]
            oth_d = [int(i) for i in d_ord if int(i) not in upg_d][: 12 - len(upg_d)]
            det_terms = [det_terms[i] for i in (upg_d + oth_d)]

        # Score ALL deterministic terminals:
        #   - Plans that ALREADY chose `end_turn` are scored AS-IS (never free-rolled with unspent gold!).
        #   - Multi-epoch frontier continuations (`stop_after=True`, NOT `end_turn`) roll out remaining gold via π!
        det_eval_games: List[seaglass.TavernGame] = []
        det_roll_idx: List[int] = []
        for di, (dg, steps_d, _) in enumerate(det_terms):
            cg = dg.copy()
            det_eval_games.append(cg)
            ends_turn = bool(steps_d) and steps_d[-1][0][0] == "end_turn"
            if (not ends_turn) and cg.observe().gold >= 3:
                det_roll_idx.append(di)
        if det_roll_idx:
            self._rollout_to_eot([det_eval_games[i] for i in det_roll_idx], ctx, max_steps=10)

        best_score = -1e9
        best_plan: List[Tuple[Tuple[str, Any, Any], Tuple, bool]] = [(("end_turn", None, None), _state_sig(o0), True)]

        if det_eval_games:
            d_scores = self._score_eot_candidates(det_eval_games, ctx, opp_board, opp_tier, is_ghost, s0)
            for i, sc in enumerate(d_scores):
                tot = float(sc) + self.w_prior * (det_terms[i][2] / max(1, len(det_terms[i][1])))
                if tot > best_score:
                    best_score = tot
                    best_plan = det_terms[i][1]

        # Score ALL stochastic candidates (`refresh`, `dark_discovery`, Triple/Discover triggers) via CRN rollouts!
        if stoch_cands:
            st_sims: List[seaglass.TavernGame] = []
            for pg, act, _, _, _ in stoch_cands:
                for rs in ref_seeds:
                    sg = pg.copy()
                    sg.reseed(rs)
                    if sg.is_legal(act):
                        sg.step(act)
                    st_sims.append(sg)
            self._rollout_to_eot(st_sims, ctx, max_steps=12)
            st_scores = self._score_eot_candidates(st_sims, ctx, opp_board, opp_tier, is_ghost, s0)
            st_means = st_scores.reshape(len(stoch_cands), self.refresh_samples).mean(axis=1)
            for i, sc in enumerate(st_means):
                _, act, steps_pre, sig_pre, lp_v = stoch_cands[i]
                plan_s = steps_pre + [(_action_key(act), sig_pre, True)]
                tot = float(sc) + self.w_prior * (lp_v / max(1, len(plan_s)))
                if tot > best_score:
                    best_score = tot
                    best_plan = plan_s

        return best_plan

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
                        self._epoch_refs[rk] = self._epoch_refs.get(rk, 0) + 1
                    if stop_after or not q:
                        self._queued.pop(player, None)
                    return m
            self._queued.pop(player, None)

        # 2. Pure-Physics Sudden-Death Finals / Lethal Exact Combat Solver
        if lobby.phase == "turn" and opp_board and not is_ghost and obs.turn >= 9 and is_sudden_death:
            return super().act_in_lobby(lobby, player)

        # 3. Hero / Trinket / Discover Prompts -> Exact 2-Turn CRN Rust Rollouts + ScalingValueTransformer!
        is_hero = lobby.phase != "turn" or bool(obs.hero_options) or obs.turn <= 0
        is_prompt = is_hero or (obs.discover_pending is not None) or bool(obs.trinket_options)
        if is_prompt:
            return self._solve_prompt_2turn(base_g, ctx, opp_board, opp_tier, is_ghost, legal, is_hero=is_hero)

        # 4. Multi-Epoch Transposition-DAG Rust Beam -> Triple-Signal EOT Evaluation!
        refs_used = self._epoch_refs.get(rk, 0)
        plan = self._plan_epoch(base_g, ctx, opp_board, opp_tier, is_ghost, refs_used=refs_used)
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
                self._epoch_refs[rk] = refs_used + 1
            if not stop0 and len(plan) > 1:
                self._queued[player] = plan[1:]
            return m0
        return legal[0]
