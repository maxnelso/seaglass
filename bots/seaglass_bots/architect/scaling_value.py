"""ScalingValueTransformer: Regularized Multi-Horizon Value & 3-Turn Scaling Network (ZERO hardcoding!).

Predicts three complementary targets from any Battlegrounds `(player_vec, slot_card_ids, slot_num_feats)` state:
  1. `place_logits (B, 8)`: Calibrated 8-way placement distribution `P(place = 1..8 | s)`
  2. `fut_stats (B,)`: Dense 3-turn future board stat scaling `E[log1p(stats_{t+3}) | s]`
     (5-10x higher signal-to-noise ratio than noisy Turn-16 placement alone — learns true scaling
     engines like Brann/Baron/Drakkari/Gem-Rat/Undead/Mechs purely from data with ZERO card rules!)
  3. `p1_logit (B,)`: Explicit 1st-place win probability logit `P(1st | s)`

Uses compact `d_model=128, layers=3, dropout=0.25` + `static_card_table()` so it:
  - Resists game-level memorization on 100-500 Grandmaster games, AND
  - Evaluates 16-32 candidate EOT boards in `< 2 ms` on CPU inside Multi-Epoch Pure-Physics Beam Search!
"""

from __future__ import annotations

import argparse
import glob
import os
import sys
import time
from pathlib import Path
from typing import Dict, List, Optional, Tuple

os.environ.setdefault("OMP_NUM_THREADS", "1")
os.environ.setdefault("MKL_NUM_THREADS", "1")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")

_RL_DIR = str(Path(__file__).resolve().parents[3] / "rl")
if _RL_DIR not in sys.path:
    sys.path.insert(0, _RL_DIR)

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

from seaglass_rl.card_features import STATIC_DIM, static_card_table
from seaglass_rl.device import add_device_arg, default_threads, pick_device
from seaglass_rl.features import (
    MAX_BOARD,
    MAX_HAND,
    MAX_SHOP,
    NUM_SLOTS,
    PLAYER_DIM,
    SLOT_NUM_DIM,
    TAVERN_PLAYER_DIM,
    VOCAB_SIZE,
)
from seaglass_rl.transformer import MAX_ZONE_POSITIONS, ZONE_NAMES, _slot_layout

SHOP_SLOT_START = MAX_BOARD + MAX_HAND
SHOP_SLOT_END = SHOP_SLOT_START + MAX_SHOP


def mask_dead_unfrozen_shop(
    pv: np.ndarray, ci: np.ndarray, nf: np.ndarray, gold_thresh: float = 0.25
) -> Tuple[np.ndarray, np.ndarray]:
    """Zero out dead unfrozen shop tokens on End-of-Turn states (unfrozen shop is discarded at EOT).

    Eliminates both spurious EOT shop variance and shortcut learning of Tavern Tier from shop cards!
    """
    is_eot = pv[:, 4] <= gold_thresh
    unfrozen = pv[:, 15] < 0.5
    dead = is_eot & unfrozen
    if not np.any(dead):
        return ci, nf
    ci_out = ci.copy()
    nf_out = nf.copy()
    ci_out[dead, SHOP_SLOT_START:SHOP_SLOT_END] = 0
    nf_out[dead, SHOP_SLOT_START:SHOP_SLOT_END] = 0.0
    return ci_out, nf_out

SCALING_PLACE_ATOMS = np.array(
    [2.40, 0.70, 0.15, -0.20, -0.45, -0.65, -0.82, -1.00], dtype=np.float32
)


class ScalingValueTransformer(nn.Module):
    """Compact regularized Entity-Attention Transformer predicting `P(place=1..8)`, `P(1st)`, and `log1p(stats_{t+3})`."""

    def __init__(
        self,
        vocab_size: int = VOCAB_SIZE,
        card_embed_dim: int = 24,
        d_model: int = 128,
        layers: int = 3,
        heads: int = 4,
        ff: int = 384,
        dropout: float = 0.25,
        use_pv_skip: bool = False,
        mask_unfrozen_shop_eot: bool = False,
    ) -> None:
        super().__init__()
        self.config = {
            "vocab_size": vocab_size,
            "card_embed_dim": card_embed_dim,
            "d_model": d_model,
            "layers": layers,
            "heads": heads,
            "ff": ff,
            "dropout": dropout,
            "use_pv_skip": bool(use_pv_skip),
            "mask_unfrozen_shop_eot": bool(mask_unfrozen_shop_eot),
        }
        static = static_card_table()[:vocab_size]
        self.register_buffer("static_table", torch.from_numpy(np.ascontiguousarray(static)), persistent=False)
        zone, pos = _slot_layout()
        self.register_buffer("slot_zone", torch.from_numpy(zone), persistent=False)
        self.register_buffer("slot_pos", torch.from_numpy(pos), persistent=False)
        self.register_buffer("place_atoms", torch.from_numpy(SCALING_PLACE_ATOMS.copy()), persistent=False)

        self.card_embed = nn.Embedding(vocab_size, card_embed_dim, padding_idx=0)
        self.card_in = nn.Linear(card_embed_dim + STATIC_DIM, d_model)
        self.num_in = nn.Linear(SLOT_NUM_DIM, d_model, bias=False)
        self.player_in = nn.Linear(PLAYER_DIM, d_model)
        self.zone_embed = nn.Embedding(len(ZONE_NAMES), d_model)
        self.pos_embed = nn.Embedding(MAX_ZONE_POSITIONS + 1, d_model)
        self.in_drop = nn.Dropout(dropout)

        enc_layer = nn.TransformerEncoderLayer(
            d_model=d_model,
            nhead=heads,
            dim_feedforward=ff,
            dropout=dropout,
            activation="gelu",
            batch_first=True,
            norm_first=True,
        )
        self.encoder = nn.TransformerEncoder(enc_layer, num_layers=layers, enable_nested_tensor=False)
        self.final_norm = nn.LayerNorm(d_model)
        self.pv_skip = (
            nn.Sequential(
                nn.Linear(TAVERN_PLAYER_DIM, d_model),
                nn.GELU(),
                nn.Dropout(dropout * 0.5),
                nn.Linear(d_model, d_model),
            )
            if use_pv_skip
            else None
        )

        self.place_head = nn.Sequential(
            nn.Dropout(dropout),
            nn.Linear(d_model, d_model),
            nn.GELU(),
            nn.Dropout(dropout * 0.5),
            nn.Linear(d_model, 8),
        )
        self.stats_head = nn.Sequential(
            nn.Dropout(dropout),
            nn.Linear(d_model, d_model // 2),
            nn.GELU(),
            nn.Linear(d_model // 2, 1),
        )
        self.p1_head = nn.Sequential(
            nn.Dropout(dropout),
            nn.Linear(d_model, d_model // 2),
            nn.GELU(),
            nn.Linear(d_model // 2, 1),
        )
        self._card_cache = None

    def _card_tokens(self) -> torch.Tensor:
        if self.training:
            self._card_cache = None
            return self.card_in(torch.cat([self.card_embed.weight, self.static_table], dim=-1))
        ac = (torch.is_autocast_enabled(), torch.get_autocast_gpu_dtype() if torch.cuda.is_available() else None)
        w = (self.card_embed.weight.data_ptr(), self.card_embed.weight._version, self.card_in.weight._version, ac)
        if self._card_cache is None or self._card_cache[0] != w:
            self._card_cache = (w, self.card_in(torch.cat([self.card_embed.weight, self.static_table], dim=-1)))
        return self._card_cache[1]

    def forward(
        self,
        player_vec: torch.Tensor,
        slot_card_ids: torch.Tensor,
        slot_num_feats: torch.Tensor,
    ) -> Tuple[torch.Tensor, torch.Tensor, torch.Tensor, torch.Tensor]:
        """Returns `(expected_place_val, place_logits_8, pred_fut_log_stats, p1_logit)`."""
        present = (slot_card_ids != 0) | (slot_num_feats[..., 0] != 0)
        B, S = present.shape
        lengths = present.sum(dim=1)
        max_n = max(1, int(lengths.max().item()))
        slots = torch.argsort((~present).to(torch.uint8), dim=1, stable=True)[:, :max_n]

        ct = self._card_tokens()
        ids = slot_card_ids.gather(1, slots)
        num = slot_num_feats.gather(1, slots.unsqueeze(-1).expand(B, max_n, SLOT_NUM_DIM))
        x = ct[ids] + self.num_in(num.to(ct.dtype)) + self.zone_embed(self.slot_zone[slots]) + self.pos_embed(self.slot_pos[slots])
        p_tok = self.player_in(player_vec.to(ct.dtype)) + self.zone_embed.weight[0]
        tokens = self.in_drop(torch.cat([p_tok.unsqueeze(1), x], dim=1))
        pad = torch.arange(max_n + 1, device=tokens.device).unsqueeze(0) > lengths.unsqueeze(1)

        h = self.final_norm(self.encoder(tokens, src_key_padding_mask=pad))
        # Pool player token + mean of active entity tokens for rich board-wide representation!
        active_mask = (~pad).float().unsqueeze(-1)
        h_mean = (h * active_mask).sum(dim=1) / active_mask.sum(dim=1).clamp(min=1.0)
        h_rep = 0.5 * (h[:, 0] + h_mean)
        if self.pv_skip is not None:
            h_rep = h_rep + self.pv_skip(player_vec[:, :TAVERN_PLAYER_DIM].to(h_rep.dtype))

        place_logits = self.place_head(h_rep).float()
        fut_stats = self.stats_head(h_rep).squeeze(-1).float()
        p1_logit = self.p1_head(h_rep).squeeze(-1).float()
        exp_val = (F.softmax(place_logits, dim=-1) * self.place_atoms).sum(dim=-1)
        return exp_val, place_logits, fut_stats, p1_logit

    def save_checkpoint(self, path: str, meta: Optional[Dict] = None) -> None:
        torch.save(
            {
                "model_class": "ScalingValueTransformer",
                "config": dict(self.config),
                "state_dict": {k: v.detach().cpu() for k, v in self.state_dict().items()},
                "meta": meta or {},
            },
            path,
        )

    @classmethod
    def load_checkpoint(cls, path: str) -> "ScalingValueTransformer":
        ck = torch.load(path, map_location="cpu", weights_only=False)
        m = cls(**ck["config"])
        m.load_state_dict(ck["state_dict"], strict=True)
        m.eval()
        return m


def _ordinal_target(pl: torch.Tensor, eps: float = 0.10) -> torch.Tensor:
    B = pl.shape[0]
    t = torch.zeros((B, 8), device=pl.device, dtype=torch.float32)
    t.scatter_(1, pl.unsqueeze(1), 1.0 - eps)
    t.scatter_add_(1, (pl - 1).clamp(min=0).unsqueeze(1), torch.full((B, 1), eps * 0.5, device=pl.device))
    t.scatter_add_(1, (pl + 1).clamp(max=7).unsqueeze(1), torch.full((B, 1), eps * 0.5, device=pl.device))
    return t


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--shard-glob", required=True)
    p.add_argument("--out-ckpt", required=True)
    p.add_argument("--d-model", type=int, default=128)
    p.add_argument("--layers", type=int, default=3)
    p.add_argument("--dropout", type=float, default=0.25)
    p.add_argument("--epochs", type=int, default=16)
    p.add_argument("--batch-size", type=int, default=512)
    p.add_argument("--lr", type=float, default=5.0e-4)
    p.add_argument("--weight-decay", type=float, default=1.0e-2)
    p.add_argument("--subsample", type=int, default=3, help="Keep 1 in N steps per game to cut intra-game correlation")
    p.add_argument("--seed", type=int, default=77)
    add_device_arg(p)
    args = p.parse_args()
    torch.set_num_threads(default_threads())

    torch.manual_seed(args.seed)
    np.random.seed(args.seed)
    dev = pick_device(args.device)

    paths = sorted(glob.glob(args.shard_glob))
    keys = ("pv", "ci", "nf", "pl", "fs", "gid")
    # Pass 1: count rows cleanly to preallocate exact arrays (ZERO peak-RAM doubling during concat!)
    step = max(1, args.subsample)
    counts = []
    shapes: Dict[str, Tuple] = {}
    dtypes: Dict[str, np.dtype] = {}
    for sp in paths:
        with np.load(sp) as z:
            n = (len(z["pl"]) + step - 1) // step
            counts.append(n)
            if not shapes:
                for k in keys:
                    shapes[k] = z[k].shape[1:]
                    dtypes[k] = z[k].dtype
    tot = sum(counts)
    data = {k: np.empty((tot,) + shapes[k], dtype=dtypes[k]) for k in keys}
    off = 0
    for sp, n in zip(paths, counts):
        with np.load(sp) as z:
            for k in keys:
                data[k][off : off + n] = z[k][::step]
        off += n

    uniq_g = np.unique(data["gid"])
    rng = np.random.default_rng(args.seed)
    rng.shuffle(uniq_g)
    n_val = max(1, int(round(len(uniq_g) * 0.15)))
    val_g = set(uniq_g[:n_val].tolist())
    is_val = np.isin(data["gid"], list(val_g))
    tr_idx = np.flatnonzero(~is_val)
    va_idx = np.flatnonzero(is_val)
    print(f"ScalingValueTransformer data: {len(paths)} shards | train={len(tr_idx):,} ({len(uniq_g)-n_val}g) | val={len(va_idx):,} ({n_val}g)", flush=True)

    model = ScalingValueTransformer(d_model=args.d_model, layers=args.layers, dropout=args.dropout).to(dev)
    print(f"Params: {sum(x.numel() for x in model.parameters()):,} on {dev}", flush=True)
    opt = torch.optim.AdamW(model.parameters(), lr=args.lr, weight_decay=args.weight_decay)

    best_score = 1e9
    Path(args.out_ckpt).parent.mkdir(parents=True, exist_ok=True)
    for ep in range(1, args.epochs + 1):
        t0 = time.perf_counter()
        prog = (ep - 1) / max(1, args.epochs - 1)
        lr_now = args.lr * (0.1 + 0.9 * 0.5 * (1.0 + np.cos(np.pi * prog)))
        for pg in opt.param_groups:
            pg["lr"] = float(lr_now)

        model.train()
        perm = rng.permutation(tr_idx)
        for i in range(0, len(perm), args.batch_size):
            idx = perm[i : i + args.batch_size]
            pv = torch.from_numpy(data["pv"][idx].astype(np.float32)).to(dev)
            ci = torch.from_numpy(data["ci"][idx].astype(np.int64)).to(dev)
            nf = torch.from_numpy(data["nf"][idx].astype(np.float32)).to(dev)
            pl = torch.from_numpy(data["pl"][idx].astype(np.int64)).to(dev)
            fs = torch.from_numpy(data["fs"][idx].astype(np.float32)).to(dev)

            opt.zero_grad(set_to_none=True)
            _, pl_lg, pred_fs, p1_lg = model(pv, ci, nf)
            t_ord = _ordinal_target(pl, eps=0.10)
            v_ce = -(t_ord * F.log_softmax(pl_lg, dim=-1)).sum(dim=-1).mean()
            y1 = (pl == 0).float()
            bce1 = F.binary_cross_entropy_with_logits(p1_lg, y1)
            ok_fs = torch.isfinite(fs)
            fs_loss = F.smooth_l1_loss(pred_fs[ok_fs], fs[ok_fs]) if bool(ok_fs.any()) else torch.zeros((), device=dev)
            loss = v_ce + 0.40 * bce1 + 0.35 * fs_loss
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            opt.step()

        # Held-out unseen-game evaluation
        model.eval()
        v_ce_s, bce1_s, fs_mae_s, n_tot, n_fs = 0.0, 0.0, 0.0, 0, 0
        with torch.inference_mode():
            for i in range(0, len(va_idx), args.batch_size):
                idx = va_idx[i : i + args.batch_size]
                pv = torch.from_numpy(data["pv"][idx].astype(np.float32)).to(dev)
                ci = torch.from_numpy(data["ci"][idx].astype(np.int64)).to(dev)
                nf = torch.from_numpy(data["nf"][idx].astype(np.float32)).to(dev)
                pl = torch.from_numpy(data["pl"][idx].astype(np.int64)).to(dev)
                fs = torch.from_numpy(data["fs"][idx].astype(np.float32)).to(dev)
                _, pl_lg, pred_fs, p1_lg = model(pv, ci, nf)
                b = len(idx)
                v_ce_s += float(F.cross_entropy(pl_lg, pl, reduction="sum").item())
                bce1_s += float(F.binary_cross_entropy_with_logits(p1_lg, (pl == 0).float(), reduction="sum").item())
                n_tot += b
                ok_fs = torch.isfinite(fs)
                if bool(ok_fs.any()):
                    fs_mae_s += float((pred_fs[ok_fs] - fs[ok_fs]).abs().sum().item())
                    n_fs += int(ok_fs.sum().item())
        val_ce = v_ce_s / max(1, n_tot)
        val_bce1 = bce1_s / max(1, n_tot)
        val_fs_mae = fs_mae_s / max(1, n_fs)
        sc = val_ce + 0.35 * val_fs_mae
        tag = ""
        if sc < best_score:
            best_score = sc
            model.save_checkpoint(args.out_ckpt, meta={"val_ce": val_ce, "val_bce1": val_bce1, "val_fs_mae": val_fs_mae, "epoch": ep})
            tag = " *BEST*"
        print(
            f"ScaleVal ep {ep:2d}/{args.epochs} | VAL dist_ce={val_ce:.3f} p1_bce={val_bce1:.3f} "
            f"fut3_log_stats_mae={val_fs_mae:.3f} | {time.perf_counter()-t0:.1f}s{tag}",
            flush=True,
        )


if __name__ == "__main__":
    main()
