"""AlphaZero-Style 1st-Place Winner Policy Distillation (`EntityTransformer`) from Saved `.replay.pb` Games.

Extracts all non-forced, non-reposition strategic Tavern decisions from the 1st-place winner
of every faithful replay (upweighting `scaling_search` 1st-place wins 2x), splits cleanly by
game file (zero leakage), and trains a fast (<0.4ms GPU) permutation-equivariant `EntityTransformer`
policy prior `P(a | s)` to replace heuristic rollout consumers & augment Stage-1 action proposals.
"""

from __future__ import annotations

import argparse
import glob
import os
import re
import sys
import time
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path
from typing import Dict, List, Optional, Tuple

os.environ.setdefault("OMP_NUM_THREADS", "1")
os.environ.setdefault("MKL_NUM_THREADS", "1")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")

_REPO = Path("/home/maxnelso/scratch/seaglass")
sys.path.insert(0, str(_REPO / "rl"))

import numpy as np
import seaglass
import torch
import torch.nn as nn
import torch.nn.functional as F

from seaglass_rl import checkpoint
from seaglass_rl.actions import ACTION_SPACE_SIZE, ACTION_TO_INDEX, action_mask
from seaglass_rl.features import encode_observation
from seaglass_rl.transformer import EntityTransformer


def _extract_one_replay(args: Tuple[str, int, bool]) -> Optional[Dict[str, np.ndarray]]:
    path, gid, scaling_top2_only = args
    try:
        tl = seaglass.Timeline.load(path)
    except Exception:
        return None
    if not tl.is_faithful or len(tl) == 0:
        return None
    last = tl[-1]
    if last.kind != "end" or last.winner is None:
        return None
    names = [p["name"] for p in tl.header["players"]]
    if scaling_top2_only:
        # Pure ScalingSearchBot distillation (ZERO SovereignBot decisions!): extract the scaling_search
        # seat from 1st-place (weight 2.0) and 2nd-place (weight 1.0) replays only.
        bn = os.path.basename(path)
        is_p1 = "_place1_" in bn
        is_p2 = "_place2_" in bn
        if not (is_p1 or is_p2):
            return None
        sc_seats = [i for i, nm in enumerate(names) if nm.startswith("scaling_search")]
        if not sc_seats:
            return None
        target_p = sc_seats[0]
        is_scaling_win = 1 if is_p1 else 0
        row_w = 2.0 if is_p1 else 1.0
    else:
        target_p = int(last.winner)
        win_name = names[target_p] if 0 <= target_p < len(names) else ""
        is_scaling_win = 1 if win_name.startswith("scaling_search") else 0
        row_w = 2.0 if is_scaling_win else 1.0

    pvs, cis, nfs, masks, acts = [], [], [], [], []
    for frame, p, action, note in tl.decisions():
        if p != target_p:
            continue
        if note and note.get("forced"):
            continue
        if action.kind in ("reposition", "choose_hero", "toggle_freeze"):
            continue
        idx = ACTION_TO_INDEX.get((action.kind, action.args[0], action.args[1]))
        if idx is None:
            continue
        before = tl.player_at(frame - 1, p)
        obs = before.observe()
        mk = action_mask(before, prune_dominated=False, obs=obs)
        if not mk[idx] or int(mk.sum()) <= 1:
            continue
        pv, ci, nf = encode_observation(obs)
        pvs.append(pv)
        cis.append(ci)
        nfs.append(nf)
        masks.append(mk)
        acts.append(idx)

    if not acts:
        return None
    n = len(acts)
    return {
        "pv": np.asarray(pvs, dtype=np.float32),
        "ci": np.asarray(cis, dtype=np.int32),
        "nf": np.asarray(nfs, dtype=np.float16),
        "mk": np.asarray(masks, dtype=np.bool_),
        "act": np.asarray(acts, dtype=np.int64),
        "gid": np.full((n,), gid, dtype=np.int32),
        "w": np.full((n,), row_w, dtype=np.float32),
        "is_sc": np.full((n,), is_scaling_win, dtype=np.int8),
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--replay-glob", required=True, help="Comma-separated globs of .replay.pb files")
    ap.add_argument("--out-ckpt", required=True)
    ap.add_argument("--max-replays", type=int, default=700)
    ap.add_argument("--exclude-seed-min", type=int, default=60000)
    ap.add_argument("--exclude-seed-max", type=int, default=199999)
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--d-model", type=int, default=128)
    ap.add_argument("--layers", type=int, default=3)
    ap.add_argument("--heads", type=int, default=4)
    ap.add_argument("--ff", type=int, default=384)
    ap.add_argument("--epochs", type=int, default=14)
    ap.add_argument("--batch-size", type=int, default=512)
    ap.add_argument("--lr", type=float, default=6.0e-4)
    ap.add_argument("--weight-decay", type=float, default=1.0e-2)
    ap.add_argument("--seed", type=int, default=42)
    ap.add_argument("--cache-npz", type=str, default="", help="Optional path to save/load extracted dataset")
    ap.add_argument("--scaling-top2-only", action="store_true", help="Distill ONLY from scaling_search 1st/2nd-place replays (ZERO SovereignBot rows!)")
    args = ap.parse_args()

    if args.cache_npz and os.path.exists(args.cache_npz):
        print(f"Loading cached dataset from {args.cache_npz}...", flush=True)
        d = np.load(args.cache_npz)
        pv_all, ci_all, nf_all = d["pv"], d["ci"], d["nf"]
        mk_all, act_all, gid_all, w_all, sc_all = d["mk"], d["act"], d["gid"], d["w"], d["is_sc"]
        n_tot = len(act_all)
        n_games = len(np.unique(gid_all))
        n_sc_rows = int((sc_all == 1).sum())
        print(f"Loaded {n_tot:,} decisions across {n_games} games (scaling_search wins: {n_sc_rows:,} rows)", flush=True)
    else:
        paths: List[str] = []
        for pat in args.replay_glob.split(","):
            if pat.strip():
                paths.extend(sorted(glob.glob(pat.strip(), recursive=True)))
        # Deduplicate, strictly exclude benchmark holdout seeds [exclude_seed_min, exclude_seed_max],
        # and prioritize scaling_search 1st-place wins + newest mtime first.
        def _is_excluded(p: str) -> bool:
            m = re.search(r"seed(\d+)", p)
            if not m:
                return False
            s = int(m.group(1))
            return args.exclude_seed_min <= s <= args.exclude_seed_max

        all_paths = [p for p in set(paths) if not _is_excluded(p)]
        paths = sorted(
            all_paths,
            key=lambda p: (1 if "place1" in os.path.basename(p) else 0, os.path.getmtime(p)),
            reverse=True,
        )[: args.max_replays]
        print(
            f"Extracting 1st-place winner decisions from {len(paths)}/{len(all_paths)} holdout-clean replays "
            f"(excluded seeds {args.exclude_seed_min}..{args.exclude_seed_max}, {args.workers} workers)...",
            flush=True,
        )
        t0 = time.perf_counter()

        chunks: List[Dict[str, np.ndarray]] = []
        with ProcessPoolExecutor(max_workers=args.workers) as ex:
            for res in ex.map(_extract_one_replay, [(p, i, bool(args.scaling_top2_only)) for i, p in enumerate(paths)]):
                if res is not None:
                    chunks.append(res)

        if not chunks:
            raise RuntimeError("No valid replay chunks extracted!")

        n_tot = sum(len(c["act"]) for c in chunks)
        pv_all = np.empty((n_tot, *chunks[0]["pv"].shape[1:]), dtype=np.float32)
        ci_all = np.empty((n_tot, *chunks[0]["ci"].shape[1:]), dtype=np.int32)
        nf_all = np.empty((n_tot, *chunks[0]["nf"].shape[1:]), dtype=np.float16)
        mk_all = np.empty((n_tot, ACTION_SPACE_SIZE), dtype=np.bool_)
        act_all = np.empty((n_tot,), dtype=np.int64)
        gid_all = np.empty((n_tot,), dtype=np.int32)
        w_all = np.empty((n_tot,), dtype=np.float32)
        sc_all = np.empty((n_tot,), dtype=np.int8)

        pos = 0
        for c in chunks:
            k = len(c["act"])
            pv_all[pos : pos + k] = c["pv"]
            ci_all[pos : pos + k] = c["ci"]
            nf_all[pos : pos + k] = c["nf"]
            mk_all[pos : pos + k] = c["mk"]
            act_all[pos : pos + k] = c["act"]
            gid_all[pos : pos + k] = c["gid"]
            w_all[pos : pos + k] = c["w"]
            sc_all[pos : pos + k] = c["is_sc"]
            pos += k
        chunks.clear()

        n_games = len(np.unique(gid_all))
        n_sc_rows = int((sc_all == 1).sum())
        print(
            f"Extracted {n_tot:,} 1st-place decisions across {n_games} games "
            f"(scaling_search wins: {n_sc_rows:,} rows) in {time.perf_counter()-t0:.1f}s!",
            flush=True,
        )
        if args.cache_npz:
            Path(args.cache_npz).parent.mkdir(parents=True, exist_ok=True)
            np.savez(
                args.cache_npz,
                pv=pv_all, ci=ci_all, nf=nf_all, mk=mk_all, act=act_all, gid=gid_all, w=w_all, is_sc=sc_all,
            )
            print(f"Cached dataset to {args.cache_npz}", flush=True)

    rng = np.random.default_rng(args.seed)
    ug = np.unique(gid_all)
    rng.shuffle(ug)
    n_val_g = max(12, int(0.12 * len(ug)))
    val_g = set(ug[:n_val_g].tolist())
    is_val = np.isin(gid_all, list(val_g))
    tr_idx = np.flatnonzero(~is_val)
    va_idx = np.flatnonzero(is_val)
    print(f"Split: train={len(tr_idx):,} ({len(ug)-n_val_g}g) | val={len(va_idx):,} ({n_val_g}g)", flush=True)

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    torch.manual_seed(args.seed)
    model = EntityTransformer(
        d_model=args.d_model,
        layers=args.layers,
        heads=args.heads,
        ff=args.ff,
        head_dim=args.d_model,
        card_embed_dim=24,
        kind_prior=True,
        static_features=True,
        seed=args.seed,
    ).to(device)
    opt = torch.optim.AdamW(model.parameters(), lr=args.lr, weight_decay=args.weight_decay)

    best_ce = 1e9
    best_stats: Dict[str, float] = {}
    Path(args.out_ckpt).parent.mkdir(parents=True, exist_ok=True)

    for ep in range(1, args.epochs + 1):
        t_ep = time.perf_counter()
        if ep <= 2:
            lr_now = args.lr * (ep / 2.0)
        else:
            prog = (ep - 2) / max(1, args.epochs - 2)
            lr_now = args.lr * (0.05 + 0.95 * 0.5 * (1.0 + np.cos(np.pi * prog)))
        for pg in opt.param_groups:
            pg["lr"] = float(lr_now)

        model.train()
        perm = rng.permutation(tr_idx)
        tr_loss_sum = 0.0
        tr_cnt = 0
        for i in range(0, len(perm), args.batch_size):
            b = perm[i : i + args.batch_size]
            pv = torch.from_numpy(pv_all[b]).to(device, dtype=torch.float32)
            ci = torch.from_numpy(ci_all[b]).to(device, dtype=torch.long)
            nf = torch.from_numpy(nf_all[b]).to(device, dtype=torch.float32)
            mk = torch.from_numpy(mk_all[b]).to(device)
            act = torch.from_numpy(act_all[b]).to(device, dtype=torch.long)
            w = torch.from_numpy(w_all[b]).to(device, dtype=torch.float32)

            opt.zero_grad(set_to_none=True)
            logits, _ = model(pv, ci, nf, mk)
            ce_per = F.cross_entropy(logits, act, reduction="none")
            loss = (ce_per * w).sum() / w.sum().clamp(min=1.0)
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            opt.step()
            tr_loss_sum += float(ce_per.sum().item())
            tr_cnt += len(b)

        model.eval()
        ce_sum = 0.0
        top1 = top3 = top5 = 0
        with torch.inference_mode():
            for i in range(0, len(va_idx), args.batch_size):
                b = va_idx[i : i + args.batch_size]
                pv = torch.from_numpy(pv_all[b]).to(device, dtype=torch.float32)
                ci = torch.from_numpy(ci_all[b]).to(device, dtype=torch.long)
                nf = torch.from_numpy(nf_all[b]).to(device, dtype=torch.float32)
                mk = torch.from_numpy(mk_all[b]).to(device)
                act = torch.from_numpy(act_all[b]).to(device, dtype=torch.long)
                logits, _ = model(pv, ci, nf, mk)
                lg = logits
                ce_sum += float(F.cross_entropy(lg, act, reduction="sum").item())
                top5_idx = lg.topk(min(5, lg.shape[-1]), dim=-1).indices
                top1 += int((top5_idx[:, 0] == act).sum().item())
                top3 += int((top5_idx[:, :3] == act.unsqueeze(-1)).any(dim=-1).sum().item())
                top5 += int((top5_idx == act.unsqueeze(-1)).any(dim=-1).sum().item())
        nv = max(1, len(va_idx))
        v_ce = ce_sum / nv
        t1 = top1 / nv
        t3 = top3 / nv
        t5 = top5 / nv
        tag = ""
        if v_ce < best_ce:
            best_ce = v_ce
            best_stats = {"val_ce": v_ce, "top1": t1, "top3": t3, "top5": t5, "epoch": ep, "n_games": n_games}
            checkpoint.save(args.out_ckpt, model, iteration=ep, args=vars(args), extra=best_stats)
            tag = " *BEST*"
        print(
            f"  ep {ep:2d}/{args.epochs} | tr_ce={tr_loss_sum/max(1,tr_cnt):.4f} val_ce={v_ce:.4f} | top1={100*t1:5.1f}% top3={100*t3:5.1f}% top5={100*t5:5.1f}% | lr={lr_now:.1e} | {time.perf_counter()-t_ep:.1f}s{tag}",
            flush=True,
        )
    print(f"Saved {args.out_ckpt} | {best_stats}", flush=True)


if __name__ == "__main__":
    main()
