"""Architecture B Bots:
  1. `NeuralPolicyBot`: Pure learned `EntityTransformer` policy (~4 ms/turn, optional pure-physics EOT positioning).
  2. `PolicyZeroBot`: True AlphaZero Policy-Guided Multi-Turn Search!
     - Uses learned `EntityTransformer` policy `π(a|s)` to propose top-K candidate actions AND
       execute fast, high-skill multi-turn continuation rollouts (`Turn t -> Combat -> Turn t+1`)
       with ZERO heuristic pruning or hardcoded card tables!
     - Evaluates leaf states with `ZeroValueNet` + exact Rust forward physics (`end_turn`, `equity_against`,
       `resolve_combat_against`, `start_turn`) + 2-pass pure-physics positioning!
"""

from __future__ import annotations

import os
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
    ACTION_SPACE_SIZE,
    ACTIONS,
    END_TURN_INDEX,
    action_from_index,
    action_mask,
)
from seaglass_rl.features import encode_observation

from .zero_bot import ZeroPhysicsBot, _action_key, _state_sig
from .zero_model import DEFAULT_ZERO_NPZ_PATH


class _GameView:
    """Thin adapter giving `TavernGame` the `Seat` interface expected by `action_mask`."""

    def __init__(self, game: seaglass.TavernGame) -> None:
        self.game = game

    def observe(self):
        return self.game.observe()

    def legal_actions(self):
        return self.game.legal_actions()

    def is_legal(self, a):
        return self.game.is_legal(a)


DEFAULT_POLICY_PT_PATH = str(
    Path(__file__).resolve().parent.parent / "data" / "architect_policy_v1.pt"
)


class NeuralPolicyBot:
    """Pure end-to-end learned `EntityTransformer` policy (zero search, optional pure-physics EOT positioning)."""

    def __init__(
        self,
        policy_ckpt: Optional[str] = None,
        deterministic: bool = True,
        temperature: float = 0.0,
        physics_positioning: bool = True,
        seed: int = 2029,
        **kwargs,
    ) -> None:
        ckpt_path = policy_ckpt or DEFAULT_POLICY_PT_PATH
        ck_raw = torch.load(ckpt_path, map_location="cpu", weights_only=False)
        if ck_raw.get("model_class") == "AmberNet":
            from .gumbel_az import AmberNet

            self.model = AmberNet.from_any_checkpoint(ckpt_path)
        elif ck_raw.get("model_class") == "DistEntityTransformer":
            from .dist_transformer import DistEntityTransformer

            self.model = DistEntityTransformer.load_checkpoint(ckpt_path)
        else:
            self.model, _ = rl_ckpt.load(ckpt_path)
        self.model.eval()
        self.deterministic = deterministic
        self.temperature = float(temperature)
        self.physics_positioning = physics_positioning
        self.rng = random.Random(seed)
        self.last_note: Optional[Dict[str, float]] = None
        self._pos_helper = ZeroPhysicsBot(weights_path="", two_turn_lookahead=False, seed=seed)
        self._pos_done: Dict[int, Tuple[int, int]] = {}
        self._queued: Dict[int, List[Tuple[Tuple[str, Any, Any], Tuple]]] = {}

    def _policy_logits(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext],
        mask: np.ndarray,
    ) -> Tuple[np.ndarray, float]:
        pv, ci, nf = encode_observation(obs, ctx, arranging=False)
        dev = next(self.model.parameters()).device
        with torch.inference_mode():
            logits, val = self.model.forward(
                torch.from_numpy(pv[None, :]).to(dev),
                torch.from_numpy(ci[None, :].astype(np.int64)).to(dev),
                torch.from_numpy(nf[None, :].astype(np.float32)).to(dev),
                torch.from_numpy(mask[None, :]).to(dev),
            )
            lps = torch.log_softmax(logits[0].float(), dim=-1).detach().cpu().numpy()
            return lps, float(val[0].float().item())

    def act_in_lobby(self, lobby: seaglass.LobbyGame, player: int) -> seaglass.TavernAction:
        obs = lobby.observe(player)
        ctx = lobby.context(player) if lobby.phase == "turn" else None
        legal = lobby.legal_actions(player)
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            return legal[0]

        def _match(ak: Tuple[str, Any, Any]) -> Optional[seaglass.TavernAction]:
            for a in legal:
                if _action_key(a) == ak:
                    return a
            return None

        # Drain queued pure-physics positioning actions if any
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

        # Build legality mask cleanly
        from seaglass_rl.play import Seat

        seat = Seat(lobby, player)
        mk = action_mask(
            seat,
            allow_reposition=not self.physics_positioning,
            allow_freeze=True,
            obs=obs,
        )
        if not mk.any():
            return seaglass.TavernAction.end_turn()
        lps, _ = self._policy_logits(obs, ctx, mk)
        if self.temperature > 0.0:
            scaled = lps / self.temperature
            scaled = scaled - float(np.max(scaled))
            probs = np.exp(scaled)
            probs[~mk] = 0.0
            probs = probs / max(1e-12, probs.sum())
            a_idx = int(np.random.default_rng(self.rng.getrandbits(32)).choice(len(probs), p=probs))
        elif self.deterministic:
            a_idx = int(np.argmax(lps))
        else:
            probs = np.exp(lps)
            probs = probs / max(1e-12, probs.sum())
            a_idx = int(np.random.default_rng(self.rng.getrandbits(32)).choice(len(probs), p=probs))
        act = action_from_index(a_idx)

        # When policy chooses `end_turn`, run pure-physics Rust EOT positioning!
        if self.physics_positioning and act.kind == "end_turn" and len(obs.board) >= 2:
            ptag = (id(lobby), obs.turn)
            if self._pos_done.get(player) != ptag:
                self._pos_done[player] = ptag
                base_g = lobby.belief_tavern(player, seed=self.rng.getrandbits(63) | 1, extra_units=[])
                ob, ot, _, _ = self._pos_helper._opponent_board_and_tier(obs, ctx)
                repo = self._pos_helper._best_position_actions(base_g, ob, ot, ctx)
                if repo:
                    sim = base_g.copy()
                    steps = []
                    for ra in repo:
                        steps.append((_action_key(ra), _state_sig(sim.observe())))
                        if sim.is_legal(ra):
                            sim.step(ra)
                    steps.append((("end_turn", None, None), _state_sig(sim.observe())))
                    m0 = _match(steps[0][0])
                    if m0 is not None:
                        self._queued[player] = steps[1:]
                        return m0

        if lobby.is_legal(player, act):
            return act
        return legal[0]


class PolicyZeroBot(ZeroPhysicsBot):
    """True AlphaZero Policy + Value Multi-Turn Rollout Search Bot!

    - Proposals: Top-K legal actions ranked by learned `EntityTransformer` policy `π(a|s)`
      (PLUS explicit macro actions like `Upgrade`, `DarkDiscovery`, `Refresh`, `EndTurn` so
      search never misses a macro fork).
    - Rollouts: Fast learned policy `π(a|s)` rolls forward the remainder of Turn `t` (AND
      Turn `t+1` on macro forks!) using CRN paired seeds!
    - Value: Learned `ZeroValueNet` + exact Rust forward physics (`end_turn`, `equity_against`,
      `resolve_combat_against`, `start_turn`)!
    - Zero hardcoded card tables, zero hardcoded leveling scripts, zero heuristic pruning!
    """

    def __init__(
        self,
        policy_ckpt: str,
        weights_path: Optional[str] = None,
        top_k_actions: int = 7,
        rollout_samples: int = 3,
        two_turn_rollout: bool = True,
        combats_eval: int = 10,
        neural_alpha: float = 0.65,
        prior_bonus: float = 1.8,
        use_value_ensemble: bool = False,
        seed: int = 2029,
        **kwargs,
    ) -> None:
        super().__init__(
            weights_path=weights_path if weights_path is not None else DEFAULT_ZERO_NPZ_PATH,
            combats_eval=combats_eval,
            neural_alpha=neural_alpha,
            two_turn_lookahead=False,
            seed=seed,
        )
        self.policy, _ = rl_ckpt.load(policy_ckpt)
        self.policy.eval()
        self.top_k_actions = top_k_actions
        self.rollout_samples = rollout_samples
        self.two_turn_rollout = two_turn_rollout
        self.prior_bonus = prior_bonus
        self.use_value_ensemble = use_value_ensemble
        self._arch_pred = None
        if use_value_ensemble:
            from .hybrid import ArchitectNumpyPredictor, DEFAULT_ARCHITECT_NPZ_PATH

            if os.path.exists(DEFAULT_ARCHITECT_NPZ_PATH):
                self._arch_pred = ArchitectNumpyPredictor(
                    catalog_name="full", weights_path=DEFAULT_ARCHITECT_NPZ_PATH
                )

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
        vals, pvecs = super()._evaluate_games_batch(
            games, ctx, opp_board, opp_tier, is_ghost, opp_max_hp, combats=combats, seed_base=seed_base
        )
        if self._arch_pred is not None and self._arch_pred.is_ready:
            obs_list = [g.observe() for g in games]
            a_utils = self._arch_pred.predict_batch(obs_list, ctx=ctx, end_of_turn=True)
            alive_units = ctx.alive_units if ctx is not None else 8
            is_finals = alive_units <= 2 and not is_ghost
            w_a = 0.20 if is_finals else 0.50
            vals = [v + w_a * (2.2 * float(au)) for v, au in zip(vals, a_utils)]
        return vals, pvecs

    def _policy_rollout_batch(
        self,
        sims: List[seaglass.TavernGame],
        ctx: Optional[seaglass.LobbyContext],
        max_steps: int = 16,
    ) -> List[seaglass.TavernGame]:
        """Batched multi-game turn rollout using fast learned policy `π(a|s)` (8x faster than per-game)."""
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
                logits, _ = self.policy.forward(
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

    def _policy_top_candidates(
        self,
        base_g: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
    ) -> List[Tuple[seaglass.TavernAction, float]]:
        obs = base_g.observe()
        view = _GameView(base_g)
        mk = action_mask(view, allow_reposition=False, allow_freeze=True, obs=obs)
        if not mk.any():
            return [(seaglass.TavernAction.end_turn(), 0.0)]
        pv, ci, nf = encode_observation(obs, ctx, arranging=False)
        with torch.no_grad():
            logits, _ = self.policy.forward(
                torch.from_numpy(pv[None, :]),
                torch.from_numpy(ci[None, :].astype(np.int64)),
                torch.from_numpy(nf[None, :].astype(np.float32)),
                torch.from_numpy(mk[None, :]),
            )
            lps = torch.log_softmax(logits[0], dim=-1).numpy()
        legal_idx = np.flatnonzero(mk)
        order = legal_idx[np.argsort(lps[legal_idx])[::-1]]
        chosen: List[int] = list(order[: self.top_k_actions])
        # Always include key macro actions (`end_turn`, `upgrade_tavern`, `refresh`, `dark_discovery`) if legal!
        for extra_a in (
            seaglass.TavernAction.end_turn(),
            seaglass.TavernAction.upgrade_tavern(),
            seaglass.TavernAction.refresh(),
            seaglass.TavernAction.dark_discovery(),
        ):
            from seaglass_rl.actions import ACTION_TO_INDEX

            ei = ACTION_TO_INDEX.get((extra_a.kind, extra_a.args[0], extra_a.args[1]))
            if ei is not None and mk[ei] and ei not in chosen:
                chosen.append(ei)
        return [(action_from_index(i), float(lps[i])) for i in chosen]

    def act_in_lobby(self, lobby: seaglass.LobbyGame, player: int) -> seaglass.TavernAction:
        obs = lobby.observe(player)
        ctx = lobby.context(player) if lobby.phase == "turn" else None
        legal = lobby.legal_actions(player)
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
            return self._choose_trinket(base_g, legal, ctx, opp_board, opp_tier, is_ghost, opp_max_hp)
        if legal[0].kind == "choose_discover":
            return self._choose_discover(base_g, legal, ctx, opp_board, opp_tier, is_ghost, opp_max_hp)

        def _match(ak: Tuple[str, Any, Any]) -> Optional[seaglass.TavernAction]:
            for a in legal:
                if _action_key(a) == ak:
                    return a
            return None

        # Drain queued EOT positioning steps
        q = self._queued.get(player)
        if q:
            ak, exp_sig, stop_after = q.pop(0)
            if _state_sig(obs) == exp_sig:
                if ak[0] == "end_turn":
                    self._queued.pop(player, None)
                    return self._finish_or_position(base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, ctx)
                m = _match(ak)
                if m is not None:
                    if stop_after or not q:
                        self._queued.pop(player, None)
                    return m
            self._queued.pop(player, None)

        cands = self._policy_top_candidates(base_g, ctx)
        if len(cands) == 1:
            a0 = cands[0][0]
            if a0.kind == "end_turn":
                return self._finish_or_position(base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, ctx)
            return _match(_action_key(a0)) or legal[0]

        # Paired CRN Multi-Turn Policy Rollouts evaluated by `ZeroValueNet` + Rust Physics!
        crn = [self.rng.getrandbits(63) | 1 for _ in range(self.rollout_samples)]
        alive_units = ctx.alive_units if ctx is not None else 8
        is_finals = alive_units <= 2 and not is_ghost
        do_t2 = self.two_turn_rollout and (not is_finals) and (3 <= obs.turn <= 12)

        t0_all: List[seaglass.TavernGame] = []
        t0_need_roll: List[int] = []
        for act, _lp in cands:
            for s in crn:
                sg = base_g.copy()
                sg.reseed(s)
                need = False
                if act.kind != "end_turn" and sg.is_legal(act):
                    ended = sg.step(act)
                    need = not ended
                t0_all.append(sg)
                if need:
                    t0_need_roll.append(len(t0_all) - 1)
        if t0_need_roll:
            rolled = self._policy_rollout_batch([t0_all[i] for i in t0_need_roll], ctx, max_steps=16)
            for idx_orig, g_r in zip(t0_need_roll, rolled):
                t0_all[idx_orig] = g_r

        t1_all: List[seaglass.TavernGame] = []
        if do_t2:
            t1_alive: List[int] = []
            for idx_g, sg0 in enumerate(t0_all):
                s = crn[idx_g % len(crn)]
                sg1 = sg0.copy()
                self._fast_generic_order(sg1)
                if sg1.is_legal(seaglass.TavernAction.end_turn()):
                    sg1.end_turn()
                if opp_board:
                    sg1.resolve_combat_against(opp_board, opponent_tier=opp_tier, seed=(s ^ 0x5678) & 0x7FFFFFFF)
                if sg1.observe().health > 0:
                    sg1.start_turn()
                    t1_alive.append(len(t1_all))
                t1_all.append(sg1)
            if t1_alive:
                rolled1 = self._policy_rollout_batch([t1_all[i] for i in t1_alive], ctx, max_steps=16)
                for idx_orig, g_r in zip(t1_alive, rolled1):
                    t1_all[idx_orig] = g_r

        v0_all, _ = self._evaluate_games_batch(
            t0_all, ctx, opp_board, opp_tier, is_ghost, opp_max_hp,
            combats=max(6, self.combats_eval // 2), seed_base=obs.turn * 1013 + 7,
        )
        v1_all = (
            self._evaluate_games_batch(
                t1_all, ctx, opp_board, opp_tier, False, opp_max_hp,
                combats=max(4, self.combats_eval // 2), seed_base=obs.turn * 2029 + 13,
            )[0]
            if do_t2
            else None
        )

        best_i = 0
        best_sc = -1e18
        M = len(crn)
        for i, (_act, lp) in enumerate(cands):
            m0 = float(np.mean(v0_all[i * M : (i + 1) * M]))
            if v1_all is not None:
                m1 = float(np.mean(v1_all[i * M : (i + 1) * M]))
                sc = 0.50 * m0 + 0.50 * m1
            else:
                sc = m0
            sc += self.prior_bonus * lp
            if sc > best_sc:
                best_sc = sc
                best_i = i

        chosen_act = cands[best_i][0]
        if chosen_act.kind == "end_turn":
            return self._finish_or_position(base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, ctx)
        m = _match(_action_key(chosen_act))
        return m if m is not None else self._finish_or_position(base_g, legal, player, obs.turn, lobby_id, opp_board, opp_tier, ctx)
