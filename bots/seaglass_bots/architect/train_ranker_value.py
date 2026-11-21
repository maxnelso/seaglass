"""Within-Game, Same-Turn Pairwise Bradley-Terry Trainer for `ScalingValueTransformer` (`train_ranker_value.py`).

100% General Learned Value Architecture (ZERO hardcoded card/comp/trinket/hero rules!):
  - Trains `ScalingValueTransformer` (drop-in compatible with `ScalingValuePredictor` ensembles!) using
    **Within-Game, Same-Turn Pairwise Bradley-Terry Ranking Loss**:
        L_BT = -w_ij * log sigmoid( score(s_{g,t,winner}) - score(s_{g,t,loser}) )
  - Why Pairwise Within-Game Ranking is a game-changer for 8-player Battlegrounds:
      1. Cancels out lobby/tribe/turn variance AND game-ID memorization (`c_g - c_g == 0`)!
      2. Directly optimizes the exact decision search faces at Turn `t`:
         "Given two candidate boards in THIS lobby on THIS turn, which finishes higher / wins 1st?"
      3. Extra `2.0x` pair weight whenever `winner` finished **1st place** (`pl == 0`)!
  - Jointly trains 8-way placement CE + 1st-place BCE + dense 3-turn future-stats regression (`fs`)
    and tracks honest held-out unseen-game `pair_acc`, `top1_pair_acc`, `dist_ce`, and `fut3_mae`!
"""

from __future__ import annotations

import argparse
import glob
import os
import sys
import time
from collections import defaultdict
from pathlib import Path
from typing import Dict, List, Tuple

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

from .scaling_value import ScalingValueTransformer, _ordinal_target


def _build_turn_pairs(
    indices: np.ndarray,
    gid: np.ndarray,
    tn: np.ndarray,
    pl: np.ndarray,
    rng: np.random.Generator,
    max_pairs_per_bucket: int = 16,
) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Builds `(win_idx, lose_idx, pair_weight)` strictly within identical `(game_id, turn)` buckets!"""
    buckets: Dict[Tuple[int, int], Dict[int, List[int]]] = defaultdict(lambda: defaultdict(list))
    for i in indices:
        t = int(tn[i])
        if t <= 0:
            continue  # Skip legacy shards without turn index (`tn == 0`) for pairwise term
        buckets[(int(gid[i]), t)][int(pl[i])].append(int(i))

    w_list: List[int] = []
    l_list: List[int] = []
    wt_list: List[float] = []
    for (_, _), by_pl in buckets.items():
        pls = sorted(by_pl.keys())
        if len(pls) < 2:
            continue
        pairs_here = []
        for a_i in range(len(pls)):
            for b_i in range(a_i + 1, len(pls)):
                pa, pb = pls[a_i], pls[b_i]
                ia = by_pl[pa][int(rng.integers(len(by_pl[pa])))]
                ib = by_pl[pb][int(rng.integers(len(by_pl[pb])))]
                wt = 2.0 if pa == 0 else (1.3 if pa == 1 else 1.0)
                pairs_here.append((ia, ib, wt))
        if len(pairs_here) > max_pairs_per_bucket:
            sel = rng.choice(len(pairs_here), size=max_pairs_per_bucket, replace=False)
            pairs_here = [pairs_here[k] for k in sel]
        for ia, ib, wt in pairs_here:
            w_list.append(ia)
            l_list.append(ib)
            wt_list.append(wt)
    return (
        np.asarray(w_list, dtype=np.int64),
        np.asarray(l_list, dtype=np.int64),
        np.asarray(wt_list, dtype=np.float32),
    )


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--shard-glob", required=True)
    p.add_argument("--out-ckpt", required=True)
    p.add_argument("--d-model", type=int, default=128)
    p.add_argument("--layers", type=int, default=3)
    p.add_argument("--heads", type=int, default=4)
    p.add_argument("--ff", type=int, default=384)
    p.add_argument("--dropout", type=float, default=0.25)
    p.add_argument("--epochs", type=int, default=16)
    p.add_argument("--pair-batch", type=int, default=256, help="Number of (winner, loser) pairs per batch")
    p.add_argument("--bt-weight", type=float, default=0.80)
    p.add_argument("--lr", type=float, default=5.0e-4)
    p.add_argument("--weight-decay", type=float, default=1.2e-2)
    p.add_argument("--subsample", type=int, default=3)
    p.add_argument("--seed", type=int, default=77)
    args = p.parse_args()

    torch.manual_seed(args.seed)
    np.random.seed(args.seed)
    rng = np.random.default_rng(args.seed)
    dev = torch.device("cuda" if torch.cuda.is_available() else "cpu")

    # Load ONLY shards with real per-turn timestamps (`gm_shard_0*.npz` — skip legacy96 which has tn=0)
    paths = sorted(p_s for p_s in glob.glob(args.shard_glob) if "legacy" not in os.path.basename(p_s))
    if not paths:
        raise FileNotFoundError(f"No timestamped GM shards matching: {args.shard_glob}")

    keys = ("pv", "ci", "nf", "pl", "tn", "fs", "gid")
    step = max(1, args.subsample)
    counts: List[int] = []
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
    rng.shuffle(uniq_g)
    n_val = max(1, int(round(len(uniq_g) * 0.15)))
    val_g = set(uniq_g[:n_val].tolist())
    is_val = np.isin(data["gid"], list(val_g))
    tr_idx = np.flatnonzero(~is_val)
    va_idx = np.flatnonzero(is_val)

    va_w, va_l, va_wt = _build_turn_pairs(va_idx, data["gid"], data["tn"], data["pl"], rng, max_pairs_per_bucket=28)
    print(
        f"RankerValueTransformer: {len(paths)} shards ({len(uniq_g)}g) | "
        f"train={len(tr_idx):,} ({len(uniq_g)-n_val}g) | val={len(va_idx):,} ({n_val}g, {len(va_w):,} val pairs)",
        flush=True,
    )

    model = ScalingValueTransformer(
        d_model=args.d_model,
        layers=args.layers,
        heads=args.heads,
        ff=args.ff,
        dropout=args.dropout,
    ).to(dev)
    print(f"Params: {sum(x.numel() for x in model.parameters()):,} on {dev}", flush=True)
    opt = torch.optim.AdamW(model.parameters(), lr=args.lr, weight_decay=args.weight_decay)

    best_score = -1e9
    Path(args.out_ckpt).parent.mkdir(parents=True, exist_ok=True)

    for ep in range(1, args.epochs + 1):
        t0 = time.perf_counter()
        prog = (ep - 1) / max(1, args.epochs - 1)
        lr_now = args.lr * (0.1 + 0.9 * 0.5 * (1.0 + np.cos(np.pi * prog)))
        for pg in opt.param_groups:
            pg["lr"] = float(lr_now)

        # Fresh within-game same-turn pairs every epoch (massive free data augmentation!)
        tr_w, tr_l, tr_wt = _build_turn_pairs(tr_idx, data["gid"], data["tn"], data["pl"], rng, max_pairs_per_bucket=24)
        perm = rng.permutation(len(tr_w))

        model.train()
        for i in range(0, len(perm), args.pair_batch):
            p_ids = perm[i : i + args.pair_batch]
            wi = tr_w[p_ids]
            li = tr_l[p_ids]
            wt = torch.from_numpy(tr_wt[p_ids]).to(dev)
            both = np.concatenate([wi, li], axis=0)
            b_n = len(wi)

            pv = torch.from_numpy(data["pv"][both]).to(dev)
            ci = torch.from_numpy(data["ci"][both].astype(np.int64)).to(dev)
            nf = torch.from_numpy(data["nf"][both].astype(np.float32)).to(dev)
            pl = torch.from_numpy(data["pl"][both].astype(np.int64)).to(dev)
            fs = torch.from_numpy(data["fs"][both].astype(np.float32)).to(dev)

            opt.zero_grad(set_to_none=True)
            ev, pl_lg, pred_fs, p1_lg = model(pv, ci, nf)

            # Combined scalar rank score = E[place_atoms] + 0.5 * P(1st) + 0.15 * fut_stats
            sc = ev + 0.50 * torch.sigmoid(p1_lg) + 0.15 * pred_fs
            diff = sc[:b_n] - sc[b_n:]
            bt_loss = (F.softplus(-1.8 * diff) * wt).sum() / wt.sum().clamp(min=1.0)

            t_ord = _ordinal_target(pl, eps=0.10)
            v_ce = -(t_ord * F.log_softmax(pl_lg, dim=-1)).sum(dim=-1).mean()
            bce1 = F.binary_cross_entropy_with_logits(p1_lg, (pl == 0).float())
            ok_fs = torch.isfinite(fs)
            fs_loss = F.smooth_l1_loss(pred_fs[ok_fs], fs[ok_fs]) if bool(ok_fs.any()) else torch.zeros((), device=dev)

            loss = args.bt_weight * bt_loss + 0.65 * v_ce + 0.35 * bce1 + 0.35 * fs_loss
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            opt.step()

        # Held-out unseen-game evaluation (dist_ce + same-game same-turn pair accuracy + 1st-place pair accuracy!)
        model.eval()
        v_ce_s, fs_mae_s, n_tot, n_fs = 0.0, 0.0, 0, 0
        with torch.inference_mode():
            for i in range(0, len(va_idx), 512):
                idx = va_idx[i : i + 512]
                pv = torch.from_numpy(data["pv"][idx]).to(dev)
                ci = torch.from_numpy(data["ci"][idx].astype(np.int64)).to(dev)
                nf = torch.from_numpy(data["nf"][idx].astype(np.float32)).to(dev)
                pl = torch.from_numpy(data["pl"][idx].astype(np.int64)).to(dev)
                fs = torch.from_numpy(data["fs"][idx].astype(np.float32)).to(dev)
                _, pl_lg, pred_fs, _ = model(pv, ci, nf)
                v_ce_s += float(F.cross_entropy(pl_lg, pl, reduction="sum").item())
                n_tot += len(idx)
                ok_fs = torch.isfinite(fs)
                if bool(ok_fs.any()):
                    fs_mae_s += float((pred_fs[ok_fs] - fs[ok_fs]).abs().sum().item())
                    n_fs += int(ok_fs.sum().item())

            p_ok, p_tot, p1_ok, p1_tot = 0, 0, 0, 0
            for i in range(0, len(va_w), 256):
                wi = va_w[i : i + 256]
                li = va_l[i : i + 256]
                b_n = len(wi)
                both = np.concatenate([wi, li], axis=0)
                pv = torch.from_numpy(data["pv"][both]).to(dev)
                ci = torch.from_numpy(data["ci"][both].astype(np.int64)).to(dev)
                nf = torch.from_numpy(data["nf"][both].astype(np.float32)).to(dev)
                ev, _, pred_fs, p1_lg = model(pv, ci, nf)
                sc = (ev + 0.50 * torch.sigmoid(p1_lg) + 0.15 * pred_fs).cpu().numpy()
                win_ok = sc[:b_n] > sc[b_n:]
                p_ok += int(win_ok.sum())
                p_tot += b_n
                is_top1 = data["pl"][wi] == 0
                if bool(is_top1.any()):
                    p1_ok += int(win_ok[is_top1].sum())
                    p1_tot += int(is_top1.sum())

        val_ce = v_ce_s / max(1, n_tot)
        val_fs = fs_mae_s / max(1, n_fs)
        pair_acc = p_ok / max(1, p_tot)
        top1_acc = p1_ok / max(1, p1_tot)
        # Selection metric: prioritize unseen same-game ranking accuracy (especially 1st place!) + low dist_ce
        sel = 2.0 * top1_acc + 1.5 * pair_acc - 0.50 * val_ce - 0.25 * val_fs
        tag = ""
        if sel > best_score:
            best_score = sel
            model.save_checkpoint(
                args.out_ckpt,
                meta={
                    "val_ce": val_ce,
                    "val_fs_mae": val_fs,
                    "val_pair_acc": pair_acc,
                    "val_top1_pair_acc": top1_acc,
                    "epoch": ep,
                },
            )
            tag = " *BEST*"
        print(
            f"RankVal ep {ep:2d}/{args.epochs} | VAL dist_ce={val_ce:.3f} fut3_mae={val_fs:.3f} "
            f"| pair_acc={pair_acc:.1%} top1_pair_acc={top1_acc:.1%} | {time.perf_counter()-t0:.1f}s{tag}",
            flush=True,
        )


if __name__ == "__main__":
    main()
