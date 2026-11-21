"""DAgger-Beam (`train_dagger_beam.py`): On-Policy Iterative Training for `ScalingSearchBot`!

100% Zero-Hardcoding Learned Value + Combinatorial Multi-Epoch Rust Beam Search:
  - Problem proven in A/B tests: Off-policy `ScalingValueTransformer` (`1.778` CE, `0.352` fut3 MAE)
    gets Goodharted when given high search weight (`neural_weight >= 2.5`) inside a combinatorial beam.
  - Solution (On-Policy DAgger / ExIt for Combinatorial Beam Search):
      1. Play 4v4 lobbies (`4x ScalingSearchBot[nw=nw_k]` vs `4x SovereignBot`) in parallel, recording
         every End-of-Turn & decision state `(pv, ci, nf, place_idx, fut3_log_stats, gid)`.
      2. Every board the beam Goodharts into 7th/8th place gets labeled `pl=7, fut3=LOW`!
         Every scaling engine the beam discovers that beats 4x SovereignBot gets labeled `pl=0, fut3=HIGH`!
      3. Retrain `ScalingValueTransformer` on `[Grandmaster Tier-6 Shards + 3x-weighted On-Policy Rounds]`
         on RTX 3070 (~2.5 min/round), systematically eliminating Goodhart exploits while ramping
         `neural_weight` (`2.2 -> 3.0 -> 4.0 -> 5.0`)!
"""

from __future__ import annotations

import argparse
import glob
import math
import multiprocessing as mp
import os
import sys
import time
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path
from typing import Any, Dict, List, Tuple

os.environ.setdefault("OMP_NUM_THREADS", "1")
os.environ.setdefault("MKL_NUM_THREADS", "1")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")
os.environ.setdefault("NUMEXPR_NUM_THREADS", "1")

_RL_DIR = str(Path(__file__).resolve().parents[3] / "rl")
if _RL_DIR not in sys.path:
    sys.path.insert(0, _RL_DIR)

import numpy as np
import seaglass
import torch
import torch.nn as nn
import torch.nn.functional as F

from seaglass_rl.features import encode_observation

from .scaling_value import ScalingValueTransformer, _ordinal_target


def _collect_one_lobby(
    args: Tuple[int, int, str, float, float, float, int, int]
) -> Dict[str, Any]:
    game_idx, seed, ckpt_path, nw, p1_b, gw, beam_w, gid_offset = args
    from seaglass_bots.architect.scaling_search_bot import ScalingSearchBot
    from seaglass_bots.sovereign_search import SovereignBot

    torch.set_num_threads(1)
    # 4 ScalingSearchBot seats vs 4 SovereignBot seats (rotated by game_idx)
    challenger_seats = {(game_idx + 2 * k) % 8 for k in range(4)}
    bots = {}
    for p in range(8):
        s_seed = seed + p * 37 + 11
        if p in challenger_seats:
            b = ScalingSearchBot(
                value_ckpt=ckpt_path,
                neural_weight=nw,
                p1_bonus=p1_b,
                growth_weight=gw,
                beam_width=beam_w,
                max_epoch_depth=4,
                top_k_combat_evals=3,
                refresh_samples=2,
                num_particles=2,
                use_cuda=False,
                seed=s_seed,
            )
            b.positioner.max_candidates = 20
            b.evaluator.positioner.max_candidates = 20
            bots[p] = b
        else:
            b = SovereignBot(
                beam_width=beam_w,
                max_epoch_depth=4,
                top_k_combat_evals=3,
                refresh_samples=2,
                num_particles=2,
                seed=s_seed,
            )
            b.positioner.max_candidates = 20
            b.evaluator.positioner.max_candidates = 20
            bots[p] = b

    lobby = seaglass.LobbyGame(players=8, duos=False, seed=seed)
    # Record 1 canonical SOT + 1 canonical EOT observation per alive seat per turn (cleanest value targets!)
    per_seat_rows: Dict[int, List[Tuple[int, np.ndarray, np.ndarray, np.ndarray]]] = {p: [] for p in range(8)}
    per_seat_stats: Dict[int, Dict[int, float]] = {p: {} for p in range(8)}

    while not lobby.done:
        lobby.begin_phase()
        is_turn = lobby.phase == "turn"
        for p in list(lobby.active_players()):
            if is_turn:
                o_sot = lobby.observe(p)
                ctx = lobby.context(p)
                pv, ci, nf = encode_observation(o_sot, ctx, arranging=False)
                per_seat_rows[p].append((int(o_sot.turn), pv.astype(np.float32), ci.astype(np.int32), nf.astype(np.float16)))
            steps = 0
            while not lobby.is_done(p) and steps < 65:
                act = bots[p].act_in_lobby(lobby, p)
                if not lobby.is_legal(p, act):
                    lobby.force_end(p)
                    break
                lobby.step(p, act)
                steps += 1
            if not lobby.is_done(p):
                lobby.force_end(p)
            if is_turn and p in lobby.alive:
                o_eot = lobby.observe(p)
                ctx = lobby.context(p)
                pv, ci, nf = encode_observation(o_eot, ctx, arranging=False)
                per_seat_rows[p].append((int(o_eot.turn), pv.astype(np.float32), ci.astype(np.int32), nf.astype(np.float16)))
                per_seat_stats[p][int(o_eot.turn)] = float(sum(max(0, u.attack) + max(0, u.health) for u in o_eot.board))
        lobby.end_phase()

    places = [int(x) for x in lobby.placements]
    gid = int(gid_offset + game_idx)

    pvs, cis, nfs, pls, fss, gids = [], [], [], [], [], []
    c_places, s_places, c_tiers, c_stats = [], [], [], []
    for p in range(8):
        pl = places[p]
        pl_idx = max(0, min(7, pl - 1))
        o_fin = lobby.observe(p)
        st_fin = float(sum(max(0, u.attack) + max(0, u.health) for u in o_fin.board))
        if p in challenger_seats:
            c_places.append(pl)
            c_tiers.append(int(o_fin.tavern_tier))
            c_stats.append(st_fin)
        else:
            s_places.append(pl)

        tl = per_seat_stats[p]
        max_t = max(tl.keys()) if tl else 1
        # Record ALL challenger seats + 2 Sovereign seats (focused on killing challenger Goodhart states!)
        keep_seat = (p in challenger_seats) or (p % 4 == 0)
        if not keep_seat:
            continue
        for t_now, pv, ci, nf in per_seat_rows[p]:
            t_f = min(max_t, t_now + 3)
            fut_s = tl.get(t_f, st_fin)
            pvs.append(pv)
            cis.append(ci)
            nfs.append(nf)
            pls.append(pl_idx)
            fss.append(math.log1p(max(0.0, fut_s)))
            gids.append(gid)

    return {
        "pv": np.stack(pvs, axis=0) if pvs else np.zeros((0, 192), dtype=np.float32),
        "ci": np.stack(cis, axis=0) if cis else np.zeros((0, 101), dtype=np.int32),
        "nf": np.stack(nfs, axis=0) if nfs else np.zeros((0, 101, 99), dtype=np.float16),
        "pl": np.asarray(pls, dtype=np.int16),
        "fs": np.asarray(fss, dtype=np.float32),
        "gid": np.asarray(gids, dtype=np.int32),
        "c_places": c_places,
        "s_places": s_places,
        "c_tiers": c_tiers,
        "c_stats": c_stats,
    }


def _retrain_scaling_value(
    gm_glob: str,
    dagger_shards: List[str],
    out_ckpt: str,
    init_ckpt: str,
    d_model: int,
    layers: int,
    dropout: float,
    epochs: int,
    lr: float,
    weight_decay: float,
    gm_subsample: int,
    dagger_upsample: int,
    seed: int,
) -> Dict[str, float]:
    torch.manual_seed(seed)
    np.random.seed(seed)
    dev = torch.device("cuda" if torch.cuda.is_available() else "cpu")

    gm_paths = sorted(glob.glob(gm_glob))
    keys = ("pv", "ci", "nf", "pl", "fs", "gid")
    step_gm = max(1, gm_subsample)

    # Pass 1: count rows (exact preallocation, zero OOM!)
    entries: List[Tuple[str, int, int]] = []  # (path, step, repeat)
    for sp in gm_paths:
        entries.append((sp, step_gm, 1))
    for dp in dagger_shards:
        entries.append((dp, 1, max(1, dagger_upsample)))

    counts = []
    shapes: Dict[str, Tuple] = {}
    dtypes: Dict[str, np.dtype] = {}
    for sp, st, rep in entries:
        with np.load(sp) as z:
            n = ((len(z["pl"]) + st - 1) // st) * rep
            counts.append(n)
            if not shapes:
                for k in keys:
                    shapes[k] = z[k].shape[1:]
                    dtypes[k] = z[k].dtype
    tot = sum(counts)
    data = {k: np.empty((tot,) + shapes[k], dtype=dtypes[k]) for k in keys}
    off = 0
    for (sp, st, rep), n in zip(entries, counts):
        with np.load(sp) as z:
            chunk = {k: z[k][::st] for k in keys}
        m = len(chunk["pl"])
        for _ in range(rep):
            for k in keys:
                data[k][off : off + m] = chunk[k]
            off += m

    uniq_g = np.unique(data["gid"])
    rng = np.random.default_rng(seed)
    rng.shuffle(uniq_g)
    n_val = max(4, int(round(len(uniq_g) * 0.12)))
    val_g = set(uniq_g[:n_val].tolist())
    is_val = np.isin(data["gid"], list(val_g))
    tr_idx = np.flatnonzero(~is_val)
    va_idx = np.flatnonzero(is_val)
    print(
        f"DAgger-Beam retrain: {len(gm_paths)} GM + {len(dagger_shards)} DAgger shards | "
        f"train={len(tr_idx):,} ({len(uniq_g)-n_val}g) | val={len(va_idx):,} ({n_val}g)",
        flush=True,
    )

    if init_ckpt and os.path.exists(init_ckpt):
        model = ScalingValueTransformer.load_checkpoint(init_ckpt).to(dev)
    else:
        model = ScalingValueTransformer(d_model=d_model, layers=layers, dropout=dropout).to(dev)
    opt = torch.optim.AdamW(model.parameters(), lr=lr, weight_decay=weight_decay)

    best_sc = 1e9
    best_metrics: Dict[str, float] = {}
    Path(out_ckpt).parent.mkdir(parents=True, exist_ok=True)
    for ep in range(1, epochs + 1):
        t0 = time.perf_counter()
        prog = (ep - 1) / max(1, epochs - 1)
        lr_now = lr * (0.1 + 0.9 * 0.5 * (1.0 + np.cos(np.pi * prog)))
        for pg in opt.param_groups:
            pg["lr"] = float(lr_now)

        model.train()
        perm = rng.permutation(tr_idx)
        for i in range(0, len(perm), 512):
            idx = perm[i : i + 512]
            pv = torch.from_numpy(data["pv"][idx]).to(dev)
            ci = torch.from_numpy(data["ci"][idx].astype(np.int64)).to(dev)
            nf = torch.from_numpy(data["nf"][idx].astype(np.float32)).to(dev)
            pl = torch.from_numpy(data["pl"][idx].astype(np.int64)).to(dev)
            fs = torch.from_numpy(data["fs"][idx].astype(np.float32)).to(dev)

            opt.zero_grad(set_to_none=True)
            _, pl_lg, pred_fs, p1_lg = model(pv, ci, nf)
            t_ord = _ordinal_target(pl, eps=0.10)
            v_ce = -(t_ord * F.log_softmax(pl_lg, dim=-1)).sum(dim=-1).mean()
            bce1 = F.binary_cross_entropy_with_logits(p1_lg, (pl == 0).float())
            ok_fs = torch.isfinite(fs)
            fs_loss = F.smooth_l1_loss(pred_fs[ok_fs], fs[ok_fs]) if bool(ok_fs.any()) else torch.zeros((), device=dev)
            loss = v_ce + 0.40 * bce1 + 0.35 * fs_loss
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            opt.step()

        model.eval()
        v_ce_s = bce1_s = fs_mae_s = 0.0
        n_tot = n_fs = 0
        with torch.inference_mode():
            for i in range(0, len(va_idx), 512):
                idx = va_idx[i : i + 512]
                pv = torch.from_numpy(data["pv"][idx]).to(dev)
                ci = torch.from_numpy(data["ci"][idx].astype(np.int64)).to(dev)
                nf = torch.from_numpy(data["nf"][idx].astype(np.float32)).to(dev)
                pl = torch.from_numpy(data["pl"][idx].astype(np.int64)).to(dev)
                fs = torch.from_numpy(data["fs"][idx].astype(np.float32)).to(dev)
                _, pl_lg, pred_fs, p1_lg = model(pv, ci, nf)
                v_ce_s += float(F.cross_entropy(pl_lg, pl, reduction="sum").item())
                bce1_s += float(F.binary_cross_entropy_with_logits(p1_lg, (pl == 0).float(), reduction="sum").item())
                n_tot += len(idx)
                ok_fs = torch.isfinite(fs)
                if bool(ok_fs.any()):
                    fs_mae_s += float((pred_fs[ok_fs] - fs[ok_fs]).abs().sum().item())
                    n_fs += int(ok_fs.sum().item())
        val_ce = v_ce_s / max(1, n_tot)
        val_bce1 = bce1_s / max(1, n_tot)
        val_fs = fs_mae_s / max(1, n_fs)
        sc = val_ce + 0.35 * val_fs
        tag = ""
        if sc < best_sc:
            best_sc = sc
            best_metrics = {"val_ce": val_ce, "val_bce1": val_bce1, "val_fs": val_fs, "epoch": ep}
            model.save_checkpoint(out_ckpt, meta=best_metrics)
            tag = " *BEST*"
        print(
            f"  DAggerVal ep {ep:2d}/{epochs} | VAL dist_ce={val_ce:.3f} p1_bce={val_bce1:.3f} "
            f"fut3_mae={val_fs:.3f} | {time.perf_counter()-t0:.1f}s{tag}",
            flush=True,
        )
    return best_metrics


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--init-ckpt", required=True)
    p.add_argument("--existing-dagger-glob", default="", help="Optional prior dagger_round_*.npz shards when resuming")
    p.add_argument("--start-round", type=int, default=1)
    p.add_argument("--gm-glob", required=True)
    p.add_argument("--run-dir", required=True)
    p.add_argument("--rounds", type=int, default=5)
    p.add_argument("--games-per-round", type=int, default=24)
    p.add_argument("--workers", type=int, default=3)
    p.add_argument("--beam-width", type=int, default=6)
    p.add_argument("--nw-schedule", default="2.2,2.8,3.4,4.0,4.6")
    p.add_argument("--p1-bonus", type=float, default=14.0)
    p.add_argument("--growth-weight", type=float, default=5.0)
    p.add_argument("--epochs", type=int, default=8)
    p.add_argument("--lr", type=float, default=2.0e-4)
    p.add_argument("--gm-subsample", type=int, default=5)
    p.add_argument("--dagger-upsample", type=int, default=3)
    p.add_argument("--seed", type=int, default=3030)
    args = p.parse_args()

    run = Path(args.run_dir)
    run.mkdir(parents=True, exist_ok=True)
    nw_list = [float(x) for x in args.nw_schedule.split(",")]
    cur_ckpt = args.init_ckpt
    dagger_shards: List[str] = sorted(glob.glob(args.existing_dagger_glob)) if args.existing_dagger_glob else []
    if dagger_shards:
        print(f"Resuming DAgger-Beam with {len(dagger_shards)} existing shards from round {args.start_round}", flush=True)

    for r in range(args.start_round, args.start_round + args.rounds):
        nw = nw_list[min(r - args.start_round, len(nw_list) - 1)]
        t0 = time.perf_counter()
        print(
            f"\n=== DAgger-Beam Round {r}/{args.start_round + args.rounds - 1} | nw={nw:.2f} | games={args.games_per_round} "
            f"| beam={args.beam_width} | ckpt={os.path.basename(cur_ckpt)} ===",
            flush=True,
        )
        jobs = [
            (
                g,
                args.seed * 1000 + r * 100 + g,
                cur_ckpt,
                nw,
                args.p1_bonus,
                args.growth_weight,
                args.beam_width,
                900000 + r * 1000,
            )
            for g in range(args.games_per_round)
        ]
        pvs, cis, nfs, pls, fss, gids = [], [], [], [], [], []
        c_pl, s_pl, c_ti, c_st = [], [], [], []
        with ProcessPoolExecutor(max_workers=args.workers, mp_context=mp.get_context("spawn")) as ex:
            futs = [ex.submit(_collect_one_lobby, j) for j in jobs]
            done_n = 0
            for fut in as_completed(futs):
                res = fut.result()
                done_n += 1
                if len(res["pl"]) > 0:
                    pvs.append(res["pv"])
                    cis.append(res["ci"])
                    nfs.append(res["nf"])
                    pls.append(res["pl"])
                    fss.append(res["fs"])
                    gids.append(res["gid"])
                c_pl.extend(res["c_places"])
                s_pl.extend(res["s_places"])
                c_ti.extend(res["c_tiers"])
                c_st.extend(res["c_stats"])
                if done_n % 6 == 0 or done_n == args.games_per_round:
                    print(
                        f"  [R{r} {done_n:2d}/{args.games_per_round}] ScaleBot avg={np.mean(c_pl):.2f} "
                        f"1st={np.mean([x==1 for x in c_pl]):.1%} top4={np.mean([x<=4 for x in c_pl]):.1%} "
                        f"T={np.mean(c_ti):.2f} medS={np.median(c_st):.0f} | Sov avg={np.mean(s_pl):.2f} "
                        f"1st={np.mean([x==1 for x in s_pl]):.1%} | {time.perf_counter()-t0:.0f}s",
                        flush=True,
                    )

        shard_path = str(run / f"dagger_round_{r:02d}.npz")
        np.savez_compressed(
            shard_path,
            pv=np.concatenate(pvs, axis=0),
            ci=np.concatenate(cis, axis=0),
            nf=np.concatenate(nfs, axis=0),
            pl=np.concatenate(pls, axis=0),
            fs=np.concatenate(fss, axis=0),
            gid=np.concatenate(gids, axis=0),
        )
        dagger_shards.append(shard_path)
        print(f"Saved {shard_path} ({sum(len(x) for x in pls):,} rows)", flush=True)

        next_ckpt = str(run / f"scaling_val_dagger_r{r:02d}.pt")
        _retrain_scaling_value(
            gm_glob=args.gm_glob,
            dagger_shards=dagger_shards,
            out_ckpt=next_ckpt,
            init_ckpt=cur_ckpt,
            d_model=160,
            layers=3,
            dropout=0.25,
            epochs=args.epochs,
            lr=args.lr,
            weight_decay=2.0e-2,
            gm_subsample=args.gm_subsample,
            dagger_upsample=args.dagger_upsample,
            seed=args.seed + r * 17,
        )
        cur_ckpt = next_ckpt


if __name__ == "__main__":
    main()
