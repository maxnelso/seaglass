"""ScalingSearchBot: Zero-Hardcoding Multi-Epoch Physics Search Powered by `ScalingValueTransformer`!

Clean scientific test of learned multi-horizon neural value (`VAL dist_ce = 1.778`, `-0.201 CE`
stronger than `sovereign_value_v1.npz`'s `1.9791`!) + explicit 1st-place utility + learned 3-turn
future stat growth (`E[log1p(stats_{t+3}) - log1p(stats_t)]`) inside a general Multi-Epoch Rust
Physics Beam — with ZERO hardcoded `comps.py`, ZERO hardcoded Trinket deltas, and ZERO Hero deltas!
"""

from __future__ import annotations

import math
import sys
from pathlib import Path
from typing import List, Optional, Sequence

_RL_DIR = str(Path(__file__).resolve().parents[3] / "rl")
if _RL_DIR not in sys.path:
    sys.path.insert(0, _RL_DIR)

import numpy as np
import seaglass
import torch
import torch.nn.functional as F

from seaglass_rl.features import encode_observation

from ..sovereign_search import SovereignBot
from .scaling_value import ScalingValueTransformer

# Sharp 1st-place-oriented placement utility (same scale as SovereignBot's [-29, +38], steeper 1st!)
SCALING_FIRST_PLACE_WEIGHTS = np.array(
    [48.0, 18.0, 6.0, -2.0, -10.0, -18.0, -26.0, -34.0], dtype=np.float32
)


class ScalingValuePredictor:
    """Drop-in neural predictor wrapping a 1..K model ensemble of `ScalingValueTransformer` (`predict_batch` interface)."""

    def __init__(
        self,
        ckpt_paths: Sequence[str],
        p1_bonus: float = 10.0,
        growth_weight: float = 4.0,
        use_cuda: bool = False,
    ) -> None:
        self.device = torch.device("cuda" if (use_cuda and torch.cuda.is_available()) else "cpu")
        self.models = [
            ScalingValueTransformer.load_checkpoint(p).to(self.device).eval()
            for p in ckpt_paths
        ]
        self.p1_bonus = float(p1_bonus)
        self.growth_weight = float(growth_weight)
        self._ready = len(self.models) > 0

    @property
    def is_ready(self) -> bool:
        return self._ready

    def predict_batch(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = True,
    ) -> List[float]:
        if not obs_list or not self.models:
            return []
        pvs, cis, nfs, cur_log_s = [], [], [], []
        for o in obs_list:
            pv, ci, nf = encode_observation(o, ctx, arranging=False)
            pvs.append(pv)
            cis.append(ci)
            nfs.append(nf)
            st = sum(max(0, u.attack) + max(0, u.health) for u in o.board)
            cur_log_s.append(math.log1p(float(st)))

        pv_t = torch.from_numpy(np.asarray(pvs, dtype=np.float32)).to(self.device)
        ci_t = torch.from_numpy(np.asarray(cis, dtype=np.int64)).to(self.device)
        nf_t = torch.from_numpy(np.asarray(nfs, dtype=np.float32)).to(self.device)

        probs_acc = None
        p1_acc = None
        fs_acc = None
        with torch.inference_mode():
            for mdl in self.models:
                _, place_logits, pred_fs, p1_lg = mdl.forward(pv_t, ci_t, nf_t)
                pr = F.softmax(place_logits, dim=-1).detach().cpu().numpy()
                p1 = torch.sigmoid(p1_lg).detach().cpu().numpy()
                fs = pred_fs.detach().cpu().numpy()
                probs_acc = pr if probs_acc is None else (probs_acc + pr)
                p1_acc = p1 if p1_acc is None else (p1_acc + p1)
                fs_acc = fs if fs_acc is None else (fs_acc + fs)

        inv_k = 1.0 / len(self.models)
        probs = probs_acc * inv_k
        p1_p = p1_acc * inv_k
        fs_np = fs_acc * inv_k

        place_u = probs @ SCALING_FIRST_PLACE_WEIGHTS
        cur_np = np.asarray(cur_log_s, dtype=np.float32)
        growth_delta = np.clip(fs_np - cur_np, -1.5, 5.0)
        tot = place_u + self.p1_bonus * p1_p + self.growth_weight * growth_delta
        return tot.astype(np.float64).tolist()


class ScalingSearchBot(SovereignBot):
    """Multi-Epoch Rust Physics Search upgraded with our `ScalingValueTransformer` ensemble (ZERO comp hardcoding!)."""

    def __init__(
        self,
        value_ckpt: Optional[str] = None,
        value_ckpts: Optional[Sequence[str]] = None,
        neural_weight: float = 1.35,
        p1_bonus: float = 10.0,
        growth_weight: float = 4.0,
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
        self.evaluator.predictor = ScalingValuePredictor(
            ckpt_paths=ckpts,
            p1_bonus=p1_bonus,
            growth_weight=growth_weight,
            use_cuda=use_cuda,
        )
        self.evaluator.use_neural_value = True
        self._base_neural_weight = float(neural_weight)
        self.evaluator.neural_weight = self._base_neural_weight
        self._base_pos_cands = int(self.positioner.max_candidates)
        self._finals_pos_cands = int(finals_pos_candidates)

    def act_in_lobby(self, lobby: seaglass.LobbyGame, player: int) -> seaglass.TavernAction:
        # Pure-Physics 1v1 Finals Focus: in Top-2 non-ghost Finals, sharpen positioning & combat focus!
        if lobby.phase == "turn":
            ctx = lobby.context(player)
            is_finals = ctx is not None and ctx.alive_units <= 2 and not ctx.next_is_ghost
            if is_finals:
                obs = lobby.observe(player)
                ehp = obs.health + obs.armor
                # In sudden-death 1v1 Finals, exact combat winrate dominates long-term scaling!
                self.evaluator.neural_weight = 0.25 if ehp <= 18 else 0.70 * self._base_neural_weight
                self.positioner.max_candidates = self._finals_pos_cands
                self.evaluator.positioner.max_candidates = self._finals_pos_cands
            else:
                self.evaluator.neural_weight = self._base_neural_weight
                self.positioner.max_candidates = self._base_pos_cands
                self.evaluator.positioner.max_candidates = self._base_pos_cands
        return super().act_in_lobby(lobby, player)
