"""PPOAlphaSearchBot: Pure Policy-Aligned AlphaZero Search over Rust Simulator!

ZERO hardcoded card tables, ZERO hardcoded hero/trinket tables, ZERO hardcoded leveling curves,
ZERO `SovereignBot` code!

Core Architecture (True AlphaZero):
  1. Single unified `EntityTransformer` trained via AWBC + 1st-Place Unbiased CUDA PPO over
     millions of RL decisions, providing BOTH policy prior `π*(a|s)` AND policy-aligned
     1st-place value Critic `V_π*(s)`.
  2. At every Tavern decision (and ChooseTrinket / ChooseDiscover / ChooseHero):
       - Proposes top-K legal actions from `π*(a|s)` (plus key macro actions if legal).
       - Simulates exact Rust state transitions + batched CRN-paired policy rollouts to
         End-of-Turn (and through exact Rust Combat -> Turn t+1 Start-of-Turn on macro forks!).
       - Evaluates leaves using the policy's OWN trained Critic `V_π*(s)` + exact Rust
         combat survival/win equity (`equity_against`)!
  3. End-of-Turn 2-pass pure-physics Rust `equity_against` combat positioning.
"""

from __future__ import annotations

import random
import sys
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

from .policy_bot import DEFAULT_POLICY_PT_PATH, _GameView
from .zero_bot import ZeroPhysicsBot, _action_key, _state_sig


class PPOAlphaSearchBot:
    """Pure AlphaZero Policy-Aligned Search: `π*(a|s)` + `V_π*(s)` + Exact Rust Simulator!"""

    def __init__(
        self,
        policy_ckpt: Optional[str] = None,
        top_k: int = 5,
        stoch_samples: int = 4,
        combat_weight: float = 0.28,
        prior_weight: float = 0.08,
        two_turn_macro: bool = True,
        combats_eval: int = 10,
        seed: int = 2029,
        **kwargs,
    ) -> None:
        ckpt_path = policy_ckpt or DEFAULT_POLICY_PT_PATH
        self.model, _ = rl_ckpt.load(ckpt_path)
        self.model.eval()
        self.top_k = top_k
        self.stoch_samples = stoch_samples
        self.combat_weight = combat_weight
        self.prior_weight = prior_weight
        self.two_turn_macro = two_turn_macro
        self.combats_eval = combats_eval
        self.rng = random.Random(seed)
        self.last_note: Optional[Dict[str, float]] = None

        self._helper = ZeroPhysicsBot(weights_path="", two_turn_lookahead=False, seed=seed)
        self._pos_done: Dict[int, Tuple[int, int]] = {}
        self._queued: Dict[int, List[Tuple[Tuple[str, Any, Any], Tuple]]] = {}
        self._ref_count: Dict[Tuple[int, int, int], int] = {}

    def _batch_eval_critic(
        self,
        games: Sequence[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
    ) -> np.ndarray:
        """Evaluates a batch of Rust `TavernGame` states using `EntityTransformer`'s own Critic `V_π*(s)`."""
        if not games:
            return np.zeros(0, dtype=np.float32)
        pvs, cis, nfs = [], [], []
        for g in games:
            obs = g.observe()
            pv, ci, nf = encode_observation(obs, ctx, arranging=False)
            pvs.append(pv)
            cis.append(ci)
            nfs.append(nf)
        with torch.no_grad():
            _, vals = self.model.forward(
                torch.from_numpy(np.stack(pvs, axis=0).astype(np.float32)),
                torch.from_numpy(np.stack(cis, axis=0).astype(np.int64)),
                torch.from_numpy(np.stack(nfs, axis=0).astype(np.float32)),
                None,
            )
            return vals.numpy().astype(np.float32)

    def _rollout_turn_batch(
        self,
        sims: List[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
        max_steps: int = 18,
    ) -> List[seaglass.TavernGame]:
        """Batched greedy rollout under `π*(a|s)` until End Turn."""
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

    def _score_eot_batch(
        self,
        games: List[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
        opp_board: List[seaglass.Unit],
        opp_tier: int,
        is_ghost: bool,
        seed_base: int,
    ) -> np.ndarray:
        """
        Pure AlphaZero leaf evaluation:
          1. Exact Rust `end_turn()` -> `equity_against()` (win/draw/lethal) -> `resolve_combat_against()` -> `start_turn()`
          2. Policy-aligned Critic `V_π*(s_eot)` + `V_π*(s_sot_next)`!
        """
        if not games:
            return np.zeros(0, dtype=np.float32)

        alive_units = ctx.alive_units if ctx is not None else 8
        is_finals = alive_units <= 2 and not is_ghost

        v_eot = self._batch_eval_critic(games, ctx)
        combat_bon = np.zeros(len(games), dtype=np.float32)
        sot_games: List[seaglass.TavernGame] = []
        sot_idx: List[int] = []
        dead_pen = np.zeros(len(games), dtype=np.float32)

        for i, g in enumerate(games):
            o0 = g.observe()
            ehp = float(o0.health + o0.armor)
            sim = g.copy()
            s_i = (seed_base + i * 97) | 1
            sim.reseed(s_i)
            self._helper._fast_generic_order(sim)
            if sim.is_legal(seaglass.TavernAction.end_turn()):
                sim.end_turn()
            if opp_board and not is_ghost:
                dist = sim.equity_against(
                    opp_board,
                    opponent_tier=opp_tier,
                    n=max(6, self.combats_eval),
                    base_seed=(s_i ^ 0x51A7) | 1,
                )
                win_r = float(dist.a_win_rate)
                draw_r = float(dist.draw_rate)
                dmg_m = float(dist.damage_mean)
                if is_finals:
                    combat_bon[i] = 2.8 * win_r + 0.9 * draw_r + 0.03 * dmg_m
                else:
                    hp_urg = 1.0 + max(0.0, (22.0 - ehp) / 14.0)
                    combat_bon[i] = self.combat_weight * hp_urg * (win_r + 0.35 * draw_r + 0.035 * dmg_m)
                sim.resolve_combat_against(opp_board, opponent_tier=opp_tier, seed=(s_i ^ 0x9E37) | 1)
            elif opp_board:
                sim.resolve_combat_against(opp_board, opponent_tier=opp_tier, seed=(s_i ^ 0x9E37) | 1)

            if sim.observe().health <= 0:
                dead_pen[i] = -1.6
            else:
                sim.start_turn()
                sot_games.append(sim)
                sot_idx.append(i)

        v_sot = np.zeros(len(games), dtype=np.float32)
        if sot_games:
            vs = self._batch_eval_critic(sot_games, ctx)
            for k, orig_i in enumerate(sot_idx):
                v_sot[orig_i] = vs[k]

        if is_finals:
            return 0.25 * v_eot + combat_bon + 0.35 * dead_pen
        return 0.45 * v_eot + 0.55 * v_sot + combat_bon + dead_pen

    def _top_candidates(
        self,
        base_g: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        refs_so_far: int,
    ) -> List[Tuple[seaglass.TavernAction, float]]:
        obs = base_g.observe()
        view = _GameView(base_g)
        mk = action_mask(view, allow_reposition=False, allow_freeze=True, obs=obs)
        if not mk.any():
            return [(seaglass.TavernAction.end_turn(), 0.0)]
        pv, ci, nf = encode_observation(obs, ctx, arranging=False)
        with torch.no_grad():
            logits, _ = self.model.forward(
                torch.from_numpy(pv[None, :]),
                torch.from_numpy(ci[None, :].astype(np.int64)),
                torch.from_numpy(nf[None, :].astype(np.float32)),
                torch.from_numpy(mk[None, :]),
            )
            lps = torch.log_softmax(logits[0], dim=-1).numpy()

        legal_idx = np.flatnonzero(mk)
        order = legal_idx[np.argsort(lps[legal_idx])[::-1]]
        chosen: List[int] = []
        for idx in order:
            a = action_from_index(int(idx))
            if a.kind == "refresh" and refs_so_far >= 10:
                continue
            chosen.append(int(idx))
            if len(chosen) >= self.top_k:
                break
        # Ensure key macro alternatives (`upgrade`, `dark_discovery`, `end_turn`) are always considered if legal
        for extra_a in (
            seaglass.TavernAction.upgrade_tavern(),
            seaglass.TavernAction.dark_discovery(),
            seaglass.TavernAction.end_turn(),
        ):
            ei = ACTION_TO_INDEX.get((extra_a.kind, extra_a.args[0], extra_a.args[1]))
            if ei is not None and mk[ei] and ei not in chosen:
                chosen.append(ei)
        return [(action_from_index(i), float(lps[i])) for i in chosen]

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

        # Drain queued pure-physics EOT positioning steps
        q = self._queued.get(player)
        if q:
            ak, exp_sig = q.pop(0)
            if _state_sig(obs) == exp_sig:
                m = _match(ak)
                if m is not None:
                    if not q:
                        self._queued.pop(player, None)
                    return m
            self._queued.pop(player, None)

        base_g = lobby.belief_tavern(player, seed=self.rng.getrandbits(63) | 1, extra_units=[])
        lobby_id = id(lobby)
        opp_board, opp_tier, is_ghost, _ = self._helper._opponent_board_and_tier(obs, ctx)

        # Prompt phases: Hero -> policy argmax; Trinket & Discover -> CRN Policy Rollouts + Critic V_π*!
        if legal[0].kind == "choose_hero":
            view = _GameView(base_g)
            mk = action_mask(view, allow_reposition=False, allow_freeze=False, obs=obs)
            pv, ci, nf = encode_observation(obs, ctx, arranging=False)
            with torch.no_grad():
                logits, _ = self.model.forward(
                    torch.from_numpy(pv[None, :]),
                    torch.from_numpy(ci[None, :].astype(np.int64)),
                    torch.from_numpy(nf[None, :].astype(np.float32)),
                    torch.from_numpy(mk[None, :]),
                )
            act = action_from_index(int(logits[0].argmax().item()))
            return _match(_action_key(act)) or legal[0]

        if legal[0].kind in ("choose_trinket", "choose_discover"):
            M = self.stoch_samples if legal[0].kind == "choose_trinket" else 2
            crn = [self.rng.getrandbits(63) | 1 for _ in range(M)]
            sims: List[seaglass.TavernGame] = []
            for a in legal:
                for s in crn:
                    sg = base_g.copy()
                    sg.reseed(s)
                    if sg.is_legal(a):
                        sg.step(a)
                    sims.append(sg)
            sims = self._rollout_turn_batch(sims, ctx, max_steps=16)
            scores = self._score_eot_batch(sims, ctx, opp_board, opp_tier, is_ghost, seed_base=obs.turn * 1009 + 3)
            best_i = int(np.argmax([float(np.mean(scores[i * M : (i + 1) * M])) for i in range(len(legal))]))
            return legal[best_i]

        # Normal Tavern Decision: AlphaZero Rollout Search over top-K policy proposals!
        rk = (lobby_id, player, obs.turn)
        refs_so_far = self._ref_count.get(rk, 0)
        cands = self._top_candidates(base_g, ctx, refs_so_far)

        # Fast-path: if top policy action has >= 92% confidence and is NOT a macro decision (upgrade/dd/refresh/end_turn), take it immediately!
        if len(cands) == 1 or (
            np.exp(cands[0][1]) >= 0.92
            and cands[0][0].kind not in ("upgrade_tavern", "dark_discovery", "refresh", "end_turn")
        ):
            a0 = cands[0][0]
            if a0.kind == "end_turn":
                return self._finish_with_positioning(base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, ctx)
            return _match(_action_key(a0)) or legal[0]

        # Paired CRN rollouts: M samples for stochastic actions (refresh/dd), 2 samples for deterministic actions
        M = max(2, self.stoch_samples)
        crn = [self.rng.getrandbits(63) | 1 for _ in range(M)]
        sims: List[seaglass.TavernGame] = []
        need_roll: List[int] = []
        slices: List[Tuple[int, int]] = []
        for act, _lp in cands:
            k_m = M if act.kind in ("refresh", "dark_discovery", "hero_power", "hero_power_2") else 2
            s0 = len(sims)
            for s in crn[:k_m]:
                sg = base_g.copy()
                sg.reseed(s)
                need = False
                if act.kind != "end_turn" and sg.is_legal(act):
                    ended = sg.step(act)
                    need = not ended
                sims.append(sg)
                if need:
                    need_roll.append(len(sims) - 1)
            slices.append((s0, len(sims)))

        if need_roll:
            rolled = self._rollout_turn_batch([sims[i] for i in need_roll], ctx, max_steps=16)
            for orig_i, g_r in zip(need_roll, rolled):
                sims[orig_i] = g_r

        scores = self._score_eot_batch(sims, ctx, opp_board, opp_tier, is_ghost, seed_base=obs.turn * 2027 + 11)
        cand_vals = [
            float(np.mean(scores[s0:s1])) + self.prior_weight * lp
            for (s0, s1), (_act, lp) in zip(slices, cands)
        ]
        chosen = cands[int(np.argmax(cand_vals))][0]
        if chosen.kind == "refresh":
            self._ref_count[rk] = refs_so_far + 1
        if chosen.kind == "end_turn":
            return self._finish_with_positioning(base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, ctx)
        return _match(_action_key(chosen)) or legal[0]

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
                steps = []
                for ra in repo:
                    steps.append((_action_key(ra), _state_sig(sim.observe())))
                    if sim.is_legal(ra):
                        sim.step(ra)
                steps.append((("end_turn", None, None), _state_sig(sim.observe())))
                for la in legal:
                    if _action_key(la) == steps[0][0]:
                        self._queued[player] = steps[1:]
                        return la
        for la in legal:
            if la.kind == "end_turn":
                return la
        return legal[0]
