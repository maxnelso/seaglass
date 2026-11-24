"""BellmanResidualNet: Frozen-Backbone Dueling CATE Head for Learned Upgrade Horizon Correction.

Learns the exact multi-turn Bellman Residual of 1-step state-value search `V(s_{t+1})`:
    r_t = U_true(s_{t+1}) - V_curr(s_{t+1})
using a frozen `ScalingValueTransformer` encoder + compact regularized Dueling MLP on
`[h_frozen(s_t), pv(s_t)]`, predicting `(res_stay(s_t), adv_upg(s_t))` in neural utility units.
ZERO hardcoded turn/tier/gold/HP/card rules; ZERO double-counting!
"""

from __future__ import annotations

import functools
from pathlib import Path
from typing import Dict, Optional, Sequence, Tuple

import numpy as np
import seaglass
import torch
import torch.nn as nn

from seaglass_rl.features import PLAYER_DIM, TAVERN_PLAYER_DIM, encode_observation

from .scaling_value import ScalingValueTransformer


def extract_backbone_pooled(
    backbone: ScalingValueTransformer,
    player_vec: torch.Tensor,
    slot_card_ids: torch.Tensor,
    slot_num_feats: torch.Tensor,
) -> torch.Tensor:
    """Extract the pooled representation `h (B, d_model)` from a `ScalingValueTransformer`."""
    present = (slot_card_ids != 0) | (slot_num_feats[..., 0] != 0)
    card_tok = backbone._card_tokens()[slot_card_ids]
    tok = (
        card_tok
        + backbone.num_in(slot_num_feats)
        + backbone.zone_embed(backbone.slot_zone)
        + backbone.pos_embed(backbone.slot_pos)
    )
    p_tok = backbone.player_in(player_vec).unsqueeze(1)
    x = torch.cat([p_tok, tok], dim=1)
    bsz = player_vec.shape[0]
    pad_mask = torch.cat([torch.zeros((bsz, 1), dtype=torch.bool, device=x.device), ~present], dim=1)
    h = backbone.final_norm(backbone.encoder(x, src_key_padding_mask=pad_mask))
    p_h = h[:, 0]
    mask_f = present.unsqueeze(-1).to(h.dtype)
    m_h = (h[:, 1:] * mask_f).sum(dim=1) / mask_f.sum(dim=1).clamp(min=1.0)
    pooled = 0.5 * (p_h + m_h)
    if backbone.pv_skip is not None:
        pooled = pooled + backbone.pv_skip(player_vec[:, :TAVERN_PLAYER_DIM])
    return pooled


class DuelingResidualHead(nn.Module):
    """Compact regularized Dueling CATE MLP over `[h_frozen, pv]` (~18k params — resists overfitting!)."""

    def __init__(
        self,
        in_dim: int,
        hidden: int = 64,
        dropout: float = 0.35,
    ) -> None:
        super().__init__()
        self.config = {"in_dim": int(in_dim), "hidden": int(hidden), "dropout": float(dropout)}
        self.in_norm = nn.LayerNorm(in_dim)
        self.base_head = nn.Sequential(
            nn.Dropout(dropout),
            nn.Linear(in_dim, hidden),
            nn.GELU(),
            nn.Dropout(dropout * 0.5),
            nn.Linear(hidden, 1),
        )
        self.adv_head = nn.Sequential(
            nn.Dropout(dropout),
            nn.Linear(in_dim, hidden),
            nn.GELU(),
            nn.Dropout(dropout * 0.5),
            nn.Linear(hidden, 1),
        )
        nn.init.zeros_(self.adv_head[-1].weight)
        nn.init.zeros_(self.adv_head[-1].bias)

    def forward(self, feat: torch.Tensor) -> Tuple[torch.Tensor, torch.Tensor]:
        x = self.in_norm(feat)
        return self.base_head(x).squeeze(-1), self.adv_head(x).squeeze(-1)

    def save_checkpoint(self, path: str, backbone_ckpt: str, meta: Optional[Dict] = None) -> None:
        Path(path).parent.mkdir(parents=True, exist_ok=True)
        torch.save(
            {
                "config": self.config,
                "backbone_ckpt": str(backbone_ckpt),
                "state_dict": self.state_dict(),
                "meta": meta or {},
            },
            path,
        )

    @classmethod
    def load_checkpoint(cls, path: str) -> Tuple["DuelingResidualHead", str]:
        ck = torch.load(path, map_location="cpu", weights_only=False)
        m = cls(**ck["config"])
        m.load_state_dict(ck["state_dict"])
        m.eval()
        return m, str(ck["backbone_ckpt"])


@functools.lru_cache(maxsize=8)
def _load_cached_backbone(path: str, device_str: str) -> ScalingValueTransformer:
    return ScalingValueTransformer.load_checkpoint(path).to(torch.device(device_str)).eval()


@functools.lru_cache(maxsize=16)
def _load_cached_head(path: str, device_str: str) -> Tuple[DuelingResidualHead, str]:
    head, bb_path = DuelingResidualHead.load_checkpoint(path)
    return head.to(torch.device(device_str)).eval(), bb_path


class UpgradeResidualPredictor:
    """Ensemble wrapper predicting learned upgrade Bellman horizon advantage `adv_upg(s_t)` in neural units."""

    def __init__(
        self,
        ckpt_paths: Sequence[str],
        use_cuda: bool = False,
        clip_abs: float = 12.0,
    ) -> None:
        self.device = torch.device("cuda" if (use_cuda and torch.cuda.is_available()) else "cpu")
        dev_str = str(self.device)
        self.heads = []
        self.backbones: Dict[str, ScalingValueTransformer] = {}
        for p in ckpt_paths:
            hd, bb_p = _load_cached_head(str(p), dev_str)
            self.heads.append((hd, bb_p))
            if bb_p not in self.backbones:
                self.backbones[bb_p] = _load_cached_backbone(bb_p, dev_str)
        self.clip_abs = float(clip_abs)

    @property
    def is_ready(self) -> bool:
        return len(self.heads) > 0

    def predict_upgrade_advantage(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> float:
        if not self.heads:
            return 0.0
        pv, ci, nf = encode_observation(obs, ctx, arranging=False)
        pv_t = torch.from_numpy(np.asarray([pv], dtype=np.float32)).to(self.device)
        ci_t = torch.from_numpy(np.asarray([ci], dtype=np.int64)).to(self.device)
        nf_t = torch.from_numpy(np.asarray([nf], dtype=np.float32)).to(self.device)
        use_bf16 = self.device.type == "cuda"
        pooled_cache: Dict[str, torch.Tensor] = {}
        acc = 0.0
        with torch.inference_mode(), torch.amp.autocast("cuda", dtype=torch.bfloat16, enabled=use_bf16):
            for hd, bb_p in self.heads:
                feat = pooled_cache.get(bb_p)
                if feat is None:
                    h = extract_backbone_pooled(self.backbones[bb_p], pv_t, ci_t, nf_t).float()
                    feat = torch.cat([h, pv_t.float()], dim=-1)
                    pooled_cache[bb_p] = feat
                _, adv = hd.forward(feat)
                acc += float(adv.float().item())
        val = acc / len(self.heads)
        return max(-self.clip_abs, min(self.clip_abs, val))
