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


def _collect_one_lobby(args: Tuple[Any, ...]) -> Dict[str, Any]:
    if len(args) == 17:
        args = (*args, 0.0, 10, 22.0, False, 4, 4, 2, "", 0.0, 1, 1, False, 1.0, False, 0.25)
    elif len(args) == 20:
        args = (*args, False, 4, 4, 2, "", 0.0, 1, 1, False, 1.0, False, 0.25)
    elif len(args) == 24:
        args = (*args, "", 0.0, 1, 1, False, 1.0, False, 0.25)
    elif len(args) == 26:
        args = (*args, 1, 1, False, 1.0, False, 0.25)
    elif len(args) == 28:
        args = (*args, False, 1.0, False, 0.25)
    (
        game_idx,
        seed,
        ckpt_path,
        nw,
        pw,
        p1_b,
        gw,
        beam_w,
        two_turn_steps,
        two_turn_w,
        n_challengers,
        gid_offset,
        unshackled_pruning,
        calibrated_rollout,
        search_noise_std,
        record_challengers_only,
        use_cuda,
        plan_explore_temp,
        plan_explore_max_turn,
        plan_explore_margin,
        opp_full_strength,
        c_depth,
        c_topk,
        c_refresh,
        upgrade_res_ckpts_str,
        upgrade_res_weight,
        two_turn_samples,
        stoch_samples,
        exact_bellman,
        combat_icm_weight,
        two_turn_unclamped,
        finals_min_nw,
    ) = args
    from seaglass_bots.architect.scaling_search_bot import ScalingSearchBot
    from seaglass_bots.sovereign_search import SovereignBot

    torch.set_num_threads(1)
    # Rotate `n_challengers` ScalingSearchBot seats vs `8 - n_challengers` SovereignBot seats
    n_c = max(1, min(8, int(n_challengers)))
    step_s = max(1, 8 // n_c) if n_c <= 4 else 1
    challenger_seats = {(game_idx + step_s * k) % 8 for k in range(n_c)}
    ckpt_list = [x.strip() for x in ckpt_path.split(",") if x.strip()]
    ures_list = [x.strip() for x in upgrade_res_ckpts_str.split(",") if x.strip()] if upgrade_res_ckpts_str else None
    bots = {}
    for p in range(8):
        s_seed = seed + p * 37 + 11
        if p in challenger_seats:
            b = ScalingSearchBot(
                value_ckpts=ckpt_list,
                upgrade_res_ckpts=ures_list,
                upgrade_res_weight=float(upgrade_res_weight),
                neural_weight=nw,
                prior_weight=pw,
                score_all_states=True,
                two_turn_rollout_steps=two_turn_steps,
                two_turn_weight=two_turn_w,
                two_turn_samples=int(two_turn_samples),
                two_turn_unclamped=bool(two_turn_unclamped),
                exact_bellman_rollout=bool(exact_bellman),
                combat_icm_weight=float(combat_icm_weight),
                finals_min_nw=float(finals_min_nw),
                stoch_samples=int(stoch_samples),
                p1_bonus=p1_b,
                growth_weight=gw,
                growth_mode="abs",
                ensemble_fast_beam=(not bool(use_cuda)),
                unshackled_pruning=unshackled_pruning,
                calibrated_rollout=calibrated_rollout,
                search_noise_std=search_noise_std,
                stratified_stage2=True,
                plan_explore_temp=plan_explore_temp,
                plan_explore_max_turn=plan_explore_max_turn,
                plan_explore_margin=plan_explore_margin,
                beam_width=beam_w,
                max_epoch_depth=c_depth,
                top_k_combat_evals=c_topk,
                refresh_samples=c_refresh,
                num_particles=2,
                use_cuda=use_cuda,
                seed=s_seed,
            )
            if not opp_full_strength:
                b.positioner.max_candidates = 20
                b.evaluator.positioner.max_candidates = 20
            bots[p] = b
        else:
            if opp_full_strength:
                b = SovereignBot(seed=s_seed)
            else:
                b = SovereignBot(
                    beam_width=beam_w,
                    max_epoch_depth=4,
                    top_k_combat_evals=4,
                    refresh_samples=2,
                    num_particles=2,
                    seed=s_seed,
                )
                b.positioner.max_candidates = 20
                b.evaluator.positioner.max_candidates = 20
            bots[p] = b

    lobby = seaglass.LobbyGame(players=8, duos=False, seed=seed)
    # Record SOT + 1 mid-turn state + EOT per alive seat per turn (calibrates ALL beam & EOT evaluations!)
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
            recorded_mid = False
            while not lobby.is_done(p) and steps < 65:
                act = bots[p].act_in_lobby(lobby, p)
                if not lobby.is_legal(p, act):
                    lobby.force_end(p)
                    break
                lobby.step(p, act)
                steps += 1
                if is_turn and not recorded_mid and steps == 2 and not lobby.is_done(p):
                    o_mid = lobby.observe(p)
                    ctx_m = lobby.context(p)
                    pv_m, ci_m, nf_m = encode_observation(o_mid, ctx_m, arranging=False)
                    per_seat_rows[p].append((int(o_mid.turn), pv_m.astype(np.float32), ci_m.astype(np.int32), nf_m.astype(np.float16)))
                    recorded_mid = True
            if not lobby.is_done(p):
                lobby.force_end(p)
            if is_turn and lobby.alive[p]:
                o_eot = lobby.observe(p)
                ctx = lobby.context(p)
                pv, ci, nf = encode_observation(o_eot, ctx, arranging=False)
                pv = pv.copy()
                pv[4] = 0.0   # Unspent EOT gold expires
                pv[10] = 0.0  # Unused once-per-turn HP1 expires
                pv[12] = 0.0
                pv[21] = 0.0  # Unused once-per-turn HP2 expires
                pv[23] = 0.0
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
        # Record challenger seats only when `record_challengers_only=True` (zero Simpson's Paradox in 2v6!)
        keep_seat = (p in challenger_seats) if record_challengers_only else ((p in challenger_seats) or (p % 4 == 0))
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
        "game_idx": game_idx,
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
    heads: int,
    ff: int,
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

    # Deterministic, leak-free game-level validation split (`gid % 10 == 0` stays held out across ALL rounds!)
    uniq_g = np.unique(data["gid"])
    rng = np.random.default_rng(seed)
    is_val = (data["gid"] % 10) == 0
    if int(is_val.sum()) < 50:
        rng.shuffle(uniq_g)
        n_val = max(4, int(round(len(uniq_g) * 0.12)))
        is_val = np.isin(data["gid"], uniq_g[:n_val].tolist())
    n_val = int(len(np.unique(data["gid"][is_val])))
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
        model = ScalingValueTransformer(
            d_model=d_model, layers=layers, heads=heads, ff=ff, dropout=dropout
        ).to(dev)
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
            with torch.amp.autocast("cuda", dtype=torch.bfloat16, enabled=(dev.type == "cuda")):
                _, pl_lg, pred_fs, p1_lg = model(pv, ci, nf)
                t_ord = _ordinal_target(pl, eps=0.12)
                v_ce = -(t_ord * F.log_softmax(pl_lg.float(), dim=-1)).sum(dim=-1).mean()
                y1_smooth = (pl == 0).float() * 0.94 + 0.03
                bce1 = F.binary_cross_entropy_with_logits(p1_lg.float(), y1_smooth)
                ok_fs = torch.isfinite(fs)
                fs_loss = (
                    F.smooth_l1_loss(pred_fs.float()[ok_fs], fs[ok_fs])
                    if bool(ok_fs.any())
                    else torch.zeros((), device=dev)
                )
                loss = v_ce + 0.45 * bce1 + 0.75 * fs_loss
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
    del model, opt
    if dev.type == "cuda":
        torch.cuda.empty_cache()
    return best_metrics


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--init-ckpt", default="", help="Optional starting checkpoint; omit to bootstrap fresh Round-0 model")
    p.add_argument("--existing-dagger-glob", default="", help="Optional prior dagger_round_*.npz shards when resuming/bootstrapping")
    p.add_argument("--start-round", type=int, default=1)
    p.add_argument("--gm-glob", required=True)
    p.add_argument("--run-dir", required=True)
    p.add_argument("--rounds", type=int, default=5)
    p.add_argument("--games-per-round", type=int, default=24)
    p.add_argument("--workers", type=int, default=3)
    p.add_argument("--beam-width", type=int, default=6)
    p.add_argument("--n-challengers", type=int, default=4)
    p.add_argument("--nw-schedule", default="2.2,2.8,3.4,4.0,4.6")
    p.add_argument("--prior-weight-schedule", default="1.0")
    p.add_argument("--two-turn-steps", type=int, default=0)
    p.add_argument("--two-turn-weight", type=float, default=0.45)
    p.add_argument("--two-turn-samples", type=int, default=1)
    p.add_argument("--two-turn-unclamped", action="store_true")
    p.add_argument("--exact-bellman", action="store_true")
    p.add_argument("--combat-icm-weight", type=float, default=1.0)
    p.add_argument("--finals-min-nw", type=float, default=0.25)
    p.add_argument("--stoch-samples", type=int, default=1)
    p.add_argument("--p1-bonus", type=float, default=14.0)
    p.add_argument("--growth-weight", type=float, default=5.0)
    p.add_argument("--d-model", type=int, default=160)
    p.add_argument("--layers", type=int, default=3)
    p.add_argument("--heads", type=int, default=4)
    p.add_argument("--ff", type=int, default=480)
    p.add_argument("--dropout", type=float, default=0.25)
    p.add_argument("--bootstrap-epochs", type=int, default=10)
    p.add_argument("--bootstrap-lr", type=float, default=3.0e-4)
    p.add_argument("--epochs", type=int, default=6)
    p.add_argument("--lr", type=float, default=1.0e-4)
    p.add_argument("--gm-subsample", type=int, default=5)
    p.add_argument("--dagger-upsample", type=int, default=3)
    p.add_argument("--unshackled-pruning", action="store_true")
    p.add_argument("--calibrated-rollout", action="store_true")
    p.add_argument("--search-noise-std", type=float, default=0.0)
    p.add_argument("--plan-explore-temp", type=float, default=0.0, help="Coherent Top-K Stage-2 plan softmax temperature (training only)")
    p.add_argument("--plan-explore-max-turn", type=int, default=10)
    p.add_argument("--plan-explore-margin", type=float, default=22.0)
    p.add_argument("--opp-full-strength", action="store_true", help="Pit ScaleBot against full-strength beam=16 SovereignBot opponents")
    p.add_argument("--challenger-depth", type=int, default=4)
    p.add_argument("--challenger-topk", type=int, default=4)
    p.add_argument("--challenger-refresh", type=int, default=2)
    p.add_argument("--upgrade-res-ckpts", default="")
    p.add_argument("--upgrade-res-weight", type=float, default=0.0)
    p.add_argument("--record-challengers-only", action="store_true")
    p.add_argument("--collect-only", action="store_true")
    p.add_argument("--retrain-from-scratch", action="store_true")
    p.add_argument("--use-cuda", action="store_true", help="Run ScalingSearchBot value inference on CUDA BF16")
    p.add_argument("--gid-base", type=int, default=900000)
    p.add_argument("--seed", type=int, default=3030)
    args = p.parse_args()

    run = Path(args.run_dir)
    run.mkdir(parents=True, exist_ok=True)
    nw_list = [float(x) for x in args.nw_schedule.split(",")]
    pw_list = [float(x) for x in args.prior_weight_schedule.split(",")]
    cur_ckpt = args.init_ckpt
    dagger_shards: List[str] = sorted(glob.glob(args.existing_dagger_glob)) if args.existing_dagger_glob else []
    if dagger_shards:
        print(f"Loaded {len(dagger_shards)} existing DAgger shards (start_round={args.start_round})", flush=True)

    if not cur_ckpt:
        boot_ckpt = str(run / "scaling_val_dagger_r00.pt")
        if os.path.exists(boot_ckpt):
            print(f"\n=== DAgger-Beam Round 0: resuming past existing {os.path.basename(boot_ckpt)} ===", flush=True)
            cur_ckpt = boot_ckpt
        else:
            print(
                f"\n=== DAgger-Beam Round 0 Bootstrap | d={args.d_model} L={args.layers} H={args.heads} FF={args.ff} ===",
                flush=True,
            )
            _retrain_scaling_value(
                gm_glob=args.gm_glob,
                dagger_shards=dagger_shards,
                out_ckpt=boot_ckpt,
                init_ckpt="",
                d_model=args.d_model,
                layers=args.layers,
                heads=args.heads,
                ff=args.ff,
                dropout=args.dropout,
                epochs=args.bootstrap_epochs,
                lr=args.bootstrap_lr,
                weight_decay=2.0e-2,
                gm_subsample=args.gm_subsample,
                dagger_upsample=args.dagger_upsample,
                seed=args.seed,
            )
            cur_ckpt = boot_ckpt

    for r in range(args.start_round, args.start_round + args.rounds):
        idx_r = r - args.start_round
        nw = nw_list[min(idx_r, len(nw_list) - 1)]
        pw = pw_list[min(idx_r, len(pw_list) - 1)]
        shard_path = str(run / f"dagger_round_{r:02d}.npz")
        next_ckpt = str(run / f"scaling_val_dagger_r{r:02d}.pt")
        if os.path.exists(shard_path) and os.path.exists(next_ckpt):
            if shard_path not in dagger_shards:
                dagger_shards.append(shard_path)
            cur_ckpt = next_ckpt
            print(f"\n=== DAgger-Beam Round {r}: resuming past completed shard & ckpt ({os.path.basename(next_ckpt)}) ===", flush=True)
            continue

        t0 = time.perf_counter()
        dev_tag = "cuda" if args.use_cuda else "cpu"
        print(
            f"\n=== DAgger-Beam Round {r}/{args.start_round + args.rounds - 1} | nw={nw:.2f} pw={pw:.2f} "
            f"| 2T={args.two_turn_steps} | games={args.games_per_round} | beam={args.beam_width} "
            f"| seats={args.n_challengers}v{8-args.n_challengers} | dev={dev_tag} | ckpt={os.path.basename(cur_ckpt)} ===",
            flush=True,
        )
        partial_dir = run / f"partial_r{r:02d}"
        partial_dir.mkdir(parents=True, exist_ok=True)
        pvs, cis, nfs, pls, fss, gids = [], [], [], [], [], []
        c_pl, s_pl, c_ti, c_st = [], [], [], []
        done_games = set()

        if os.path.exists(shard_path):
            print(f"Found completed shard {shard_path}; skipping rollout and jumping to retrain.", flush=True)
        else:
            for g in range(args.games_per_round):
                gp = partial_dir / f"g{g:03d}.npz"
                if gp.exists():
                    try:
                        with np.load(gp) as z:
                            if len(z["pl"]) > 0:
                                pvs.append(z["pv"])
                                cis.append(z["ci"])
                                nfs.append(z["nf"])
                                pls.append(z["pl"])
                                fss.append(z["fs"])
                                gids.append(z["gid"])
                            c_pl.extend(z["c_places"].tolist())
                            s_pl.extend(z["s_places"].tolist())
                            c_ti.extend(z["c_tiers"].tolist())
                            c_st.extend(z["c_stats"].tolist())
                        done_games.add(g)
                    except Exception:
                        gp.unlink(missing_ok=True)
            if done_games:
                print(f"  Resumed {len(done_games)}/{args.games_per_round} saved games from {partial_dir}", flush=True)

            jobs = [
                (
                    g,
                    args.seed * 1000 + r * 100 + g,
                    cur_ckpt,
                    nw,
                    pw,
                    args.p1_bonus,
                    args.growth_weight,
                    args.beam_width,
                    args.two_turn_steps,
                    args.two_turn_weight,
                    args.n_challengers,
                    args.gid_base + r * 1000,
                    bool(args.unshackled_pruning),
                    bool(args.calibrated_rollout),
                    float(args.search_noise_std),
                    bool(args.record_challengers_only),
                    bool(args.use_cuda),
                    float(args.plan_explore_temp),
                    int(args.plan_explore_max_turn),
                    float(args.plan_explore_margin),
                    bool(args.opp_full_strength),
                    int(args.challenger_depth),
                    int(args.challenger_topk),
                    int(args.challenger_refresh),
                    str(args.upgrade_res_ckpts),
                    float(args.upgrade_res_weight),
                    int(args.two_turn_samples),
                    int(args.stoch_samples),
                    bool(args.exact_bellman),
                    float(args.combat_icm_weight),
                    bool(args.two_turn_unclamped),
                    float(args.finals_min_nw),
                )
                for g in range(args.games_per_round)
                if g not in done_games
            ]
            done_n = len(done_games)
            if jobs:
                with ProcessPoolExecutor(max_workers=args.workers, mp_context=mp.get_context("spawn")) as ex:
                    futs = [ex.submit(_collect_one_lobby, j) for j in jobs]
                    for fut in as_completed(futs):
                        res = fut.result()
                        g_id = int(res["game_idx"])
                        tmp_gp = partial_dir / f".g{g_id:03d}.tmp.npz"
                        fin_gp = partial_dir / f"g{g_id:03d}.npz"
                        np.savez_compressed(
                            tmp_gp,
                            pv=res["pv"],
                            ci=res["ci"],
                            nf=res["nf"],
                            pl=res["pl"],
                            fs=res["fs"],
                            gid=res["gid"],
                            c_places=np.asarray(res["c_places"], dtype=np.int16),
                            s_places=np.asarray(res["s_places"], dtype=np.int16),
                            c_tiers=np.asarray(res["c_tiers"], dtype=np.int16),
                            c_stats=np.asarray(res["c_stats"], dtype=np.float32),
                        )
                        os.replace(tmp_gp, fin_gp)
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
                            s_info = (
                                f"Sov avg={np.mean(s_pl):.2f} 1st={np.mean([x==1 for x in s_pl]):.1%}"
                                if s_pl
                                else "pure-selfplay"
                            )
                            print(
                                f"  [R{r} {done_n:2d}/{args.games_per_round}] ScaleBot avg={np.mean(c_pl):.2f} "
                                f"1st={np.mean([x==1 for x in c_pl]):.1%} top4={np.mean([x<=4 for x in c_pl]):.1%} "
                                f"T={np.mean(c_ti):.2f} medS={np.median(c_st):.0f} | {s_info} | {time.perf_counter()-t0:.0f}s",
                                flush=True,
                            )

            tmp_shard = str(run / f".dagger_round_{r:02d}.tmp.npz")
            np.savez_compressed(
                tmp_shard,
                pv=np.concatenate(pvs, axis=0),
                ci=np.concatenate(cis, axis=0),
                nf=np.concatenate(nfs, axis=0),
                pl=np.concatenate(pls, axis=0),
                fs=np.concatenate(fss, axis=0),
                gid=np.concatenate(gids, axis=0),
            )
            os.replace(tmp_shard, shard_path)
            for gp in partial_dir.glob("*.npz"):
                gp.unlink(missing_ok=True)
            try:
                partial_dir.rmdir()
            except OSError:
                pass
            print(f"Saved {shard_path} ({sum(len(x) for x in pls):,} rows)", flush=True)

        if shard_path not in dagger_shards:
            dagger_shards.append(shard_path)

        if args.collect_only:
            continue

        _retrain_scaling_value(
            gm_glob=args.gm_glob,
            dagger_shards=dagger_shards,
            out_ckpt=next_ckpt,
            init_ckpt="" if args.retrain_from_scratch else cur_ckpt,
            d_model=args.d_model,
            layers=args.layers,
            heads=args.heads,
            ff=args.ff,
            dropout=args.dropout,
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
