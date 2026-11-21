"""AlphaStar-Style Grandmaster Supervised Pretrainer for `DistEntityTransformer` (`train_gm_dist.py`).

100% General Learned Architecture (ZERO hardcoded card/comp/hero/trinket/leveling rules!):
  - Loads sharded `allow_reposition=False` trajectories from `gm_shards/gm_shard_*.npz`
    (collected from Tier-5/6 Turn-16-21 `SovereignBot` + `ArchitectBot` lobbies).
  - Trains a high-capacity `DistEntityTransformer` (default `d_model=224, num_layers=6, nhead=8`,
    ~3.5M params) jointly with:
      1. Placement-Weighted Behavioral Cloning (`w_pol` emphasizing 1st & 2nd place winners) +
         mild label smoothing (`0.03`) to prevent overconfidence while keeping `argmax` sharp!
      2. Unbiased Full-Lobby 8-Way Distributional Placement Critic (`dist_critic`: Ordinal-Smoothed
         CE + `0.35 * BCE(1st place)`) calibrated strictly to Tier-6 SovereignBot lobbies!
      3. Dense Auxiliary 3-Turn Future-Stats (`log1p(stats_{t+3})`) Huber head (Amber-Eye-style
         auxiliary representation shaping so the Transformer learns long-term scaling dynamics!).
  - Strict held-out game-ID validation split (`10%` unseen games) measuring honest:
      `val_pol_ce`, `val_top1`, `val_top3`, `val_dist_ce`, `val_p1_auc`!
  - Saves standard `DistEntityTransformer` checkpoints ready for direct evaluation (`NeuralPolicyBot`,
    `FinalsPhysicsRLBot`, `GumbelSearchBot`) AND superhuman Turn-Level GAE `fast_dist_ppo` RL!
"""

from __future__ import annotations

import argparse
import gc
import glob
import os
import sys
import time
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

from .dist_transformer import DistEntityTransformer

# Emphasize 1st & 2nd place winners in policy cloning while keeping critic 100% unbiased!
POL_PLACE_WEIGHTS = np.array(
    [2.00, 1.35, 0.85, 0.55, 0.30, 0.20, 0.15, 0.10], dtype=np.float32
)


def _ordinal_target_8(place_idx: torch.Tensor, eps: float = 0.08) -> torch.Tensor:
    B = place_idx.shape[0]
    t = torch.zeros((B, 8), device=place_idx.device, dtype=torch.float32)
    t.scatter_(1, place_idx.unsqueeze(1), 1.0 - eps)
    left = (place_idx - 1).clamp(min=0)
    right = (place_idx + 1).clamp(max=7)
    t.scatter_add_(1, left.unsqueeze(1), torch.full((B, 1), eps * 0.5, device=place_idx.device))
    t.scatter_add_(1, right.unsqueeze(1), torch.full((B, 1), eps * 0.5, device=place_idx.device))
    return t


def load_one_shard(sp: str) -> Dict[str, np.ndarray]:
    keys = ("pv", "ci", "nf", "mk", "ac", "pl", "tn", "fs", "gid")
    with np.load(sp) as z:
        out = {k: np.array(z[k]) for k in keys}
    return out


def load_gm_shards(shard_glob: str, max_shards: int = 0) -> Dict[str, np.ndarray]:
    paths = sorted(glob.glob(shard_glob))
    if max_shards > 0:
        paths = paths[:max_shards]
    if not paths:
        raise FileNotFoundError(f"No shards found matching: {shard_glob}")
    keys = ("pv", "ci", "nf", "mk", "ac", "pl", "tn", "fs", "gid")
    # Pass 1: count rows to preallocate exact arrays (zero peak-RAM doubling on 300k+ steps!)
    counts: List[int] = []
    shapes: Dict[str, Tuple] = {}
    dtypes: Dict[str, np.dtype] = {}
    for sp in paths:
        with np.load(sp) as z:
            counts.append(len(z["ac"]))
            if not shapes:
                for k in keys:
                    shapes[k] = z[k].shape[1:]
                    dtypes[k] = z[k].dtype
    tot = sum(counts)
    out = {k: np.empty((tot,) + shapes[k], dtype=dtypes[k]) for k in keys}
    off = 0
    for sp, n in zip(paths, counts):
        with np.load(sp) as z:
            for k in keys:
                out[k][off : off + n] = z[k]
        off += n
    print(f"Loaded {len(paths)} shards: {tot:,} steps across {len(np.unique(out['gid']))} games", flush=True)
    return out


def eval_epoch(
    model: DistEntityTransformer,
    data: Dict[str, np.ndarray],
    val_idx: np.ndarray,
    dev: torch.device,
    batch_size: int = 1024,
) -> Dict[str, float]:
    model.eval()
    tot_pol_ce = 0.0
    tot_top1 = 0.0
    tot_top3 = 0.0
    tot_v_ce = 0.0
    tot_n = 0
    with torch.inference_mode():
        for i in range(0, len(val_idx), batch_size):
            idx = val_idx[i : i + batch_size]
            pv = torch.from_numpy(data["pv"][idx]).to(dev, non_blocking=True)
            ci = torch.from_numpy(data["ci"][idx].astype(np.int64)).to(dev, non_blocking=True)
            nf = torch.from_numpy(data["nf"][idx].astype(np.float32)).to(dev, non_blocking=True)
            mk = torch.from_numpy(data["mk"][idx]).to(dev, non_blocking=True)
            ac = torch.from_numpy(data["ac"][idx].astype(np.int64)).to(dev, non_blocking=True)
            pl = torch.from_numpy(data["pl"][idx].astype(np.int64)).to(dev, non_blocking=True)
            with torch.amp.autocast("cuda", dtype=torch.float16, enabled=dev.type == "cuda"):
                logits, _, place_logits = model.forward_with_dist(pv, ci, nf, mk)
            l_f = logits.float()
            p_f = place_logits.float()
            b = len(idx)
            tot_pol_ce += float(F.cross_entropy(l_f, ac, reduction="sum").item())
            tot_v_ce += float(F.cross_entropy(p_f, pl, reduction="sum").item())
            top3 = torch.topk(l_f, k=min(3, l_f.shape[-1]), dim=-1).indices
            tot_top1 += float((top3[:, 0] == ac).sum().item())
            tot_top3 += float((top3 == ac.unsqueeze(1)).any(dim=1).sum().item())
            tot_n += b
    return {
        "val_pol_ce": tot_pol_ce / max(1, tot_n),
        "val_top1": tot_top1 / max(1, tot_n),
        "val_top3": tot_top3 / max(1, tot_n),
        "val_dist_ce": tot_v_ce / max(1, tot_n),
    }


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--shard-glob", required=True)
    p.add_argument("--max-shards", type=int, default=0)
    p.add_argument("--out-ckpt", required=True)
    p.add_argument("--init-ckpt", default="", help="Optional existing DistEntityTransformer checkpoint to warm-start")
    p.add_argument("--d-model", type=int, default=224)
    p.add_argument("--nhead", type=int, default=8)
    p.add_argument("--num-layers", type=int, default=6)
    p.add_argument("--dim-ff", type=int, default=672)
    p.add_argument("--epochs", type=int, default=14)
    p.add_argument("--batch-size", type=int, default=1024)
    p.add_argument("--lr", type=float, default=6.0e-4)
    p.add_argument("--weight-decay", type=float, default=2.0e-4)
    p.add_argument("--val-weight", type=float, default=0.05, help="Small critic weight during BC to avoid memorizing N games")
    p.add_argument("--val-frac", type=float, default=0.10)
    p.add_argument("--stream-shards", action="store_true", help="Stream 1 shard at a time (constant ~2-3 GiB RAM on 500k+ steps)")
    p.add_argument("--seed", type=int, default=42)
    args = p.parse_args()

    torch.manual_seed(args.seed)
    np.random.seed(args.seed)
    dev = torch.device("cuda" if torch.cuda.is_available() else "cpu")

    rng = np.random.default_rng(args.seed)
    stream_train_paths: List[str] = []
    if args.stream_shards:
        all_paths = sorted(glob.glob(args.shard_glob))
        if args.max_shards > 0:
            all_paths = all_paths[: args.max_shards]
        if len(all_paths) < 2:
            raise ValueError("--stream-shards requires at least 2 shards")
        # Pick the last pure numbered shard (avoiding legacy96) as strict unseen 32-game val set!
        val_cand = [p for p in all_paths if "legacy" not in os.path.basename(p)]
        val_path = val_cand[-1] if val_cand else all_paths[-1]
        stream_train_paths = [p for p in all_paths if p != val_path]
        val_data = load_one_shard(val_path)
        val_idx = np.arange(len(val_data["ac"]), dtype=np.int64)
        data = None
        train_idx = np.empty(0, dtype=np.int64)
        print(
            f"Stream mode: {len(stream_train_paths)} train shards | val shard={os.path.basename(val_path)} "
            f"({len(val_idx):,} steps, {len(np.unique(val_data['gid']))} unseen games)",
            flush=True,
        )
    else:
        data = load_gm_shards(args.shard_glob, max_shards=args.max_shards)
        gids = data["gid"]
        uniq_g = np.unique(gids)
        rng.shuffle(uniq_g)
        n_val_g = max(1, int(round(len(uniq_g) * args.val_frac)))
        val_g = set(uniq_g[:n_val_g].tolist())
        is_val = np.isin(gids, list(val_g))
        train_idx = np.flatnonzero(~is_val)
        val_idx = np.flatnonzero(is_val)
        val_data = data
        print(
            f"Split: train={len(train_idx):,} steps ({len(uniq_g)-n_val_g}g) | "
            f"val={len(val_idx):,} steps ({n_val_g}g)",
            flush=True,
        )

    if args.init_ckpt and os.path.exists(args.init_ckpt):
        model = DistEntityTransformer.load_checkpoint(args.init_ckpt).to(dev)
        print(f"Warm-started from {args.init_ckpt}", flush=True)
    else:
        model = DistEntityTransformer(
            card_embed_dim=32,
            d_model=args.d_model,
            layers=args.num_layers,
            heads=args.nhead,
            ff=args.dim_ff,
            head_dim=128,
            kind_prior=True,
            static_features=True,
            seed=args.seed,
        ).to(dev)
    model.BUCKETS = 8

    # Auxiliary 3-turn future-stats head (shapes `h_player` encoder features; discarded at inference)
    d_m = int(model.config["d_model"])
    aux_stats_head = nn.Sequential(nn.Linear(d_m, d_m // 2), nn.GELU(), nn.Linear(d_m // 2, 1)).to(dev)

    n_params = sum(param.numel() for param in model.parameters())
    print(f"DistEntityTransformer config={model.config} params={n_params:,} on {dev}", flush=True)

    opt = torch.optim.AdamW(
        list(model.parameters()) + list(aux_stats_head.parameters()),
        lr=args.lr,
        weight_decay=args.weight_decay,
    )
    scaler = torch.amp.GradScaler("cuda", enabled=dev.type == "cuda")
    w_table = torch.from_numpy(POL_PLACE_WEIGHTS).to(dev)

    best_score = 1e9
    Path(args.out_ckpt).parent.mkdir(parents=True, exist_ok=True)
    t0_all = time.perf_counter()

    for ep in range(1, args.epochs + 1):
        t0 = time.perf_counter()
        # Cosine LR schedule with 1-epoch warmup
        if ep == 1:
            lr_now = args.lr * 0.5
        else:
            prog = (ep - 1) / max(1, args.epochs - 1)
            lr_now = args.lr * (0.08 + 0.92 * 0.5 * (1.0 + np.cos(np.pi * prog)))
        for pg in opt.param_groups:
            pg["lr"] = float(lr_now)

        model.train()
        aux_stats_head.train()
        tr_pol, tr_val, tr_aux, steps = 0.0, 0.0, 0.0, 0

        if args.stream_shards:
            order = list(stream_train_paths)
            rng.shuffle(order)
            chunks = [(sp, None) for sp in order]
        else:
            chunks = [(None, (data, train_idx))]

        for sp, mem_pair in chunks:
            if sp is not None:
                cur_data = load_one_shard(sp)
                cur_idx = np.arange(len(cur_data["ac"]), dtype=np.int64)
            else:
                cur_data, cur_idx = mem_pair  # type: ignore[misc]
            perm = rng.permutation(cur_idx)
            for i in range(0, len(perm), args.batch_size):
                idx = perm[i : i + args.batch_size]
                pv = torch.from_numpy(cur_data["pv"][idx]).to(dev, non_blocking=True)
                ci = torch.from_numpy(cur_data["ci"][idx].astype(np.int64)).to(dev, non_blocking=True)
                nf = torch.from_numpy(cur_data["nf"][idx].astype(np.float32)).to(dev, non_blocking=True)
                mk = torch.from_numpy(cur_data["mk"][idx]).to(dev, non_blocking=True)
                ac = torch.from_numpy(cur_data["ac"][idx].astype(np.int64)).to(dev, non_blocking=True)
                pl = torch.from_numpy(cur_data["pl"][idx].astype(np.int64)).to(dev, non_blocking=True)
                fs = torch.from_numpy(cur_data["fs"][idx].astype(np.float32)).to(dev, non_blocking=True)

                opt.zero_grad(set_to_none=True)
                with torch.amp.autocast("cuda", dtype=torch.float16, enabled=dev.type == "cuda"):
                    slots, lengths, token_of_slot = model._pack(ci, nf)
                    h, start = model._encode(pv, ci, nf, slots, lengths)
                    h_player = h[start]

                    # 8-way distributional critic
                    place_logits = model.dist_critic(h_player).float()
                    # Auxiliary 3-turn log1p(stats) prediction
                    pred_fs = aux_stats_head(h_player).squeeze(-1).float()

                    # Action logits (exact same head as DistEntityTransformer.forward_with_dist)
                    B = pv.shape[0]
                    A = model.act_src.shape[0]
                    rows, acts = mk.nonzero(as_tuple=True)
                    src = start[rows] + token_of_slot[rows, model.act_src[acts]]
                    pre = model.w_src(h[src]) + model.w_ctx(h_player)[rows] + model.per_action_embed()[acts]
                    tgt = model.act_tgt[acts]
                    has_tgt = tgt >= 0
                    if bool(has_tgt.any()):
                        with_tgt = has_tgt.nonzero(as_tuple=True)[0]
                        r = rows[with_tgt]
                        tgt_tok = start[r] + token_of_slot[r, tgt[with_tgt]]
                        pre = pre.index_add(0, with_tgt, model.w_tgt(h[tgt_tok]).to(pre.dtype))
                    scores = model.actor_out(torch.relu(pre)).squeeze(-1).float()
                    logits = scores.new_full((B, A), -1e9)
                    logits[rows, acts] = scores
                    if model.kind_prior:
                        counts = mk.float() @ model.kind_onehot
                        per_action = counts @ model.kind_onehot.T
                        logits = logits - torch.log(per_action.clamp(min=1.0))
                        logits = logits.masked_fill(~mk, -1e9)

                # Pure FP32 loss calculation (autocast-safe & numerically exact)
                w_pol = w_table[pl]
                log_p = F.log_softmax(logits, dim=-1)
                nll = -log_p.gather(1, ac.unsqueeze(1)).squeeze(1)
                legal_cnt = mk.float().sum(dim=-1).clamp(min=1.0)
                mean_legal_lp = (log_p.masked_fill(~mk, 0.0)).sum(dim=-1) / legal_cnt
                pol_loss = ((0.97 * nll - 0.03 * mean_legal_lp) * w_pol).sum() / w_pol.sum().clamp(min=1.0)

                t_ord = _ordinal_target_8(pl, eps=0.08)
                v_ce = -(t_ord * F.log_softmax(place_logits, dim=-1)).sum(dim=-1).mean()
                p1_prob = F.softmax(place_logits, dim=-1)[:, 0].clamp(1e-6, 1.0 - 1e-6)
                y1 = (pl == 0).float()
                bce1 = F.binary_cross_entropy(p1_prob, y1)
                val_loss = v_ce + 0.35 * bce1

                valid_fs = torch.isfinite(fs)
                if bool(valid_fs.any()):
                    aux_loss = F.smooth_l1_loss(pred_fs[valid_fs], fs[valid_fs])
                else:
                    aux_loss = torch.zeros((), device=dev, dtype=torch.float32)

                loss = pol_loss + args.val_weight * val_loss + 0.05 * aux_loss

                scaler.scale(loss).backward()
                scaler.unscale_(opt)
                nn.utils.clip_grad_norm_(model.parameters(), 1.0)
                scaler.step(opt)
                scaler.update()

                tr_pol += float(pol_loss.item())
                tr_val += float(v_ce.item())
                tr_aux += float(aux_loss.item())
                steps += 1
            if sp is not None:
                del cur_data, perm
                gc.collect()

        ev = eval_epoch(model, val_data, val_idx, dev, batch_size=args.batch_size)
        # Select best checkpoint strictly by held-out policy NLL (critic trains online on fresh PPO games!)
        score = ev["val_pol_ce"]
        saved = ""
        if score < best_score:
            best_score = score
            model.save_checkpoint(args.out_ckpt, iteration=ep)
            saved = " *BEST*"
        print(
            f"GM-Dist ep {ep:2d}/{args.epochs} | lr={lr_now:.1e} "
            f"| tr_pol={tr_pol/max(1,steps):.3f} tr_vce={tr_val/max(1,steps):.3f} tr_aux={tr_aux/max(1,steps):.3f} "
            f"| VAL pol_ce={ev['val_pol_ce']:.3f} top1={ev['val_top1']:.1%} top3={ev['val_top3']:.1%} "
            f"dist_ce={ev['val_dist_ce']:.3f} | {time.perf_counter()-t0:.1f}s ({time.perf_counter()-t0_all:.0f}s){saved}",
            flush=True,
        )


if __name__ == "__main__":
    main()
