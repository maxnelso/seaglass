"""Twin-World Paired Same-Seed Counterfactual Self-Play Collector (ZERO hardcoding!).

For each lobby seed S and target turn T_int in {2, 5, 6, 7, 8}:
  - World A (UPGRADE): Runs exact 1v7 FFA (1x ScalingSearchBot vs 7x full-strength SovereignBot).
    At the first turn >= T_int where `upgrade_tavern` is legal at start of turn, forces `upgrade_tavern`
    as the first action of that turn, then lets ScalingSearchBot play the rest of the turn & game normally.
  - World B (STAY): Starts from the EXACT SAME lobby seed S and deterministic bot seeds.
    On that exact same intervention turn, blocks `upgrade_tavern` for that single turn only
    (`block_upgrade_turn=t_actual`), then plays the rest of the game completely unrestricted.

Because World A and World B share the EXACT SAME hero, HP, armor, board, hand, shop, and 7 opponents
at the moment of divergence, the paired outcomes `(place_A, stats_A)` vs `(place_B, stats_B)` provide
a 100% UNCONFOUNDED causal treatment effect (CATE) of leveling vs staying — zero shop/hero selection bias!
"""

from __future__ import annotations

import argparse
import json
import math
import os
import sys
import time
from concurrent.futures import ProcessPoolExecutor, as_completed
from pathlib import Path
from typing import Dict, List, Optional, Tuple

os.environ.setdefault("OMP_NUM_THREADS", "1")
os.environ.setdefault("MKL_NUM_THREADS", "1")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")

_REPO = Path("/home/maxnelso/scratch/seaglass")
sys.path.insert(0, str(_REPO / "bots"))
sys.path.insert(0, str(_REPO / "rl"))

import numpy as np
import torch
import seaglass

from seaglass_rl.features import encode_observation


def _run_world(
    seed: int,
    seat: int,
    bot_kwargs: dict,
    target_turn: int,
    mode: str,  # "upgrade" or "stay"
    forced_actual_turn: Optional[int] = None,
) -> Tuple[Optional[int], int, float, List[Tuple[int, np.ndarray, np.ndarray, np.ndarray]], Dict[int, float], float, int]:
    """Run one 1v7 lobby world; returns (actual_turn, place, fin_stats, rows, turn_stats, ehp_at_int, tier_before)."""
    from seaglass_bots.architect.scaling_search_bot import ScalingSearchBot
    from seaglass_bots.sovereign_search import SovereignBot

    torch.set_num_threads(1)
    bots = {}
    for p in range(8):
        s_seed = seed + p * 37 + 11
        if p == seat:
            kw = dict(bot_kwargs)
            kw["seed"] = s_seed
            if mode == "stay" and forced_actual_turn is not None:
                kw["block_upgrade_turn"] = int(forced_actual_turn)
            bots[p] = ScalingSearchBot(**kw)
        else:
            b = SovereignBot(
                beam_width=10,
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
    rows: List[Tuple[int, np.ndarray, np.ndarray, np.ndarray]] = []
    turn_stats: Dict[int, float] = {}
    actual_turn: Optional[int] = forced_actual_turn
    did_intervene = False
    ehp_at_int = 0.0
    tier_before = 0

    while not lobby.done:
        lobby.begin_phase()
        is_turn = lobby.phase == "turn"
        for p in list(lobby.active_players()):
            if p == seat and is_turn:
                o_sot = lobby.observe(p)
                t_now = int(o_sot.turn)
                if mode == "stay" and actual_turn is not None and t_now == actual_turn and not did_intervene:
                    ehp_at_int = float(o_sot.health + o_sot.armor)
                    tier_before = int(o_sot.tavern_tier)
                    did_intervene = True

                # Record SOT (World A records all turns; World B records only turns >= actual_turn)
                if mode == "upgrade" or (actual_turn is not None and t_now >= actual_turn):
                    ctx = lobby.context(p)
                    pv, ci, nf = encode_observation(o_sot, ctx, arranging=False)
                    rows.append((t_now, pv.astype(np.float32), ci.astype(np.int32), nf.astype(np.float16)))

            steps = 0
            recorded_mid = False
            while not lobby.is_done(p) and steps < 65:
                # In "upgrade" mode, fire UPGRADE as soon as legal at start of turn (even after Turn 6/9 Trinket pick!)
                if (
                    p == seat
                    and is_turn
                    and mode == "upgrade"
                    and not did_intervene
                    and steps <= 2
                ):
                    o_chk = lobby.observe(p)
                    t_chk = int(o_chk.turn)
                    if target_turn <= t_chk <= target_turn + 3:
                        upg_act = next((a for a in lobby.legal_actions(p) if a.kind == "upgrade_tavern"), None)
                        if upg_act is not None:
                            actual_turn = t_chk
                            ehp_at_int = float(o_chk.health + o_chk.armor)
                            tier_before = int(o_chk.tavern_tier)
                            lobby.step(p, upg_act)
                            did_intervene = True
                            steps += 1
                            continue
                act = bots[p].act_in_lobby(lobby, p)
                if not lobby.is_legal(p, act):
                    lobby.force_end(p)
                    break
                lobby.step(p, act)
                steps += 1
                if p == seat and is_turn and not recorded_mid and steps == 2 and not lobby.is_done(p):
                    o_mid = lobby.observe(p)
                    t_m = int(o_mid.turn)
                    if mode == "upgrade" or (actual_turn is not None and t_m >= actual_turn):
                        ctx_m = lobby.context(p)
                        pv_m, ci_m, nf_m = encode_observation(o_mid, ctx_m, arranging=False)
                        rows.append((t_m, pv_m.astype(np.float32), ci_m.astype(np.int32), nf_m.astype(np.float16)))
                    recorded_mid = True
            if not lobby.is_done(p):
                lobby.force_end(p)

            if p == seat and is_turn and lobby.alive[p]:
                o_eot = lobby.observe(p)
                t_e = int(o_eot.turn)
                turn_stats[t_e] = float(sum(max(0, u.attack) + max(0, u.health) for u in o_eot.board))
                if mode == "upgrade" or (actual_turn is not None and t_e >= actual_turn):
                    ctx = lobby.context(p)
                    pv, ci, nf = encode_observation(o_eot, ctx, arranging=False)
                    pv = pv.copy()
                    pv[4] = 0.0
                    pv[10] = 0.0
                    pv[12] = 0.0
                    pv[21] = 0.0
                    pv[23] = 0.0
                    rows.append((t_e, pv.astype(np.float32), ci.astype(np.int32), nf.astype(np.float16)))
        lobby.end_phase()

    pl = int(lobby.placements[seat])
    o_fin = lobby.observe(seat)
    st_fin = float(sum(max(0, u.attack) + max(0, u.health) for u in o_fin.board))
    return actual_turn, pl, st_fin, rows, turn_stats, ehp_at_int, tier_before


def _pack_rows(
    rows: List[Tuple[int, np.ndarray, np.ndarray, np.ndarray]],
    turn_stats: Dict[int, float],
    pl: int,
    st_fin: float,
    gid: int,
):
    pl_idx = max(0, min(7, pl - 1))
    max_t = max(turn_stats.keys()) if turn_stats else 1
    pvs, cis, nfs, pls, fss, gids = [], [], [], [], [], []
    for t_now, pv, ci, nf in rows:
        t_f = min(max_t, t_now + 3)
        fut_s = turn_stats.get(t_f, st_fin)
        pvs.append(pv)
        cis.append(ci)
        nfs.append(nf)
        pls.append(pl_idx)
        fss.append(math.log1p(max(0.0, fut_s)))
        gids.append(gid)
    return pvs, cis, nfs, pls, fss, gids


def _run_twin_pair(args):
    pair_idx, seed, target_turn, bot_kwargs, gid_offset = args
    t0 = time.perf_counter()
    seat = pair_idx % 8
    gid = int(gid_offset + pair_idx)

    # World A: Force UPGRADE at first legal turn >= target_turn
    act_t, pl_u, st_u, rows_u, ts_u, ehp_int, tier_bef = _run_world(
        seed=seed, seat=seat, bot_kwargs=bot_kwargs, target_turn=target_turn, mode="upgrade"
    )
    if act_t is None:
        return {"pair_idx": pair_idx, "skipped": True, "wall": time.perf_counter() - t0}

    # World B: EXACT SAME SEED, block UPGRADE on that exact turn `act_t`
    _, pl_s, st_s, rows_s, ts_s, _, _ = _run_world(
        seed=seed,
        seat=seat,
        bot_kwargs=bot_kwargs,
        target_turn=target_turn,
        mode="stay",
        forced_actual_turn=act_t,
    )

    pvs_u, cis_u, nfs_u, pls_u, fss_u, gids_u = _pack_rows(rows_u, ts_u, pl_u, st_u, gid)
    pvs_s, cis_s, nfs_s, pls_s, fss_s, gids_s = _pack_rows(rows_s, ts_s, pl_s, st_s, gid)

    pvs = pvs_u + pvs_s
    cis = cis_u + cis_s
    nfs = nfs_u + nfs_s
    pls = pls_u + pls_s
    fss = fss_u + fss_s
    gids = gids_u + gids_s

    fs_u_t = math.log1p(max(0.0, ts_u.get(min(max(ts_u.keys(), default=1), act_t + 3), st_u)))
    fs_s_t = math.log1p(max(0.0, ts_s.get(min(max(ts_s.keys(), default=1), act_t + 3), st_s)))

    return {
        "pair_idx": pair_idx,
        "skipped": False,
        "seed": seed,
        "seat": seat,
        "target_turn": target_turn,
        "actual_turn": act_t,
        "tier_before": tier_bef,
        "ehp_at_int": ehp_int,
        "pl_upg": pl_u,
        "pl_stay": pl_s,
        "fs_upg": fs_u_t,
        "fs_stay": fs_s_t,
        "wall": time.perf_counter() - t0,
        "pv": np.stack(pvs, axis=0),
        "ci": np.stack(cis, axis=0),
        "nf": np.stack(nfs, axis=0),
        "pl": np.asarray(pls, dtype=np.int16),
        "fs": np.asarray(fss, dtype=np.float32),
        "gid": np.asarray(gids, dtype=np.int32),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out-npz", required=True)
    ap.add_argument("--out-json", required=True)
    ap.add_argument("--kwargs", required=True, help="JSON kwargs for ScalingSearchBot")
    ap.add_argument("--pairs", type=int, default=32)
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--base-seed", type=int, default=810000)
    ap.add_argument("--gid-offset", type=int, default=810000)
    ap.add_argument("--turns", type=str, default="2,5,6,7,8")
    args = ap.parse_args()

    bot_kwargs = json.loads(args.kwargs)
    turns_cycle = [int(x) for x in args.turns.split(",") if x.strip()]

    jobs = []
    for i in range(args.pairs):
        seed = args.base_seed + i * 1009
        tt = turns_cycle[i % len(turns_cycle)]
        jobs.append((i, seed, tt, bot_kwargs, args.gid_offset))

    print(
        f"Launching Twin-World Counterfactual RCT: {args.pairs} pairs ({args.pairs*2} 1v7 lobbies), "
        f"workers={args.workers}, turns={turns_cycle}, base_seed={args.base_seed}"
    )
    t_start = time.perf_counter()
    results = []
    pvs, cis, nfs, pls, fss, gids = [], [], [], [], [], []

    with ProcessPoolExecutor(max_workers=args.workers) as ex:
        futs = [ex.submit(_run_twin_pair, j) for j in jobs]
        done_n = 0
        for fut in as_completed(futs):
            res = fut.result()
            done_n += 1
            if res.get("skipped"):
                print(f"[{done_n:2d}/{args.pairs}] pair={res['pair_idx']:02d} SKIPPED (wall={res['wall']:.1f}s)")
                continue
            pvs.append(res["pv"])
            cis.append(res["ci"])
            nfs.append(res["nf"])
            pls.append(res["pl"])
            fss.append(res["fs"])
            gids.append(res["gid"])
            d_pl = res["pl_stay"] - res["pl_upg"]  # positive = UPGRADE beat STAY!
            d_fs = res["fs_upg"] - res["fs_stay"]
            meta = {k: v for k, v in res.items() if k not in ("pv", "ci", "nf", "pl", "fs", "gid")}
            results.append(meta)
            print(
                f"[{done_n:2d}/{args.pairs}] pair={res['pair_idx']:02d} T{res['actual_turn']} (t{res['tier_before']}->{res['tier_before']+1}, ehp={res['ehp_at_int']:.0f}) | "
                f"UPG=P{res['pl_upg']} vs STAY=P{res['pl_stay']} (dPL={d_pl:+d}, dFS={d_fs:+.2f}) | rows={len(res['pl'])} wall={res['wall']:.1f}s"
            )

    Path(args.out_npz).parent.mkdir(parents=True, exist_ok=True)
    if pvs:
        np.savez_compressed(
            args.out_npz,
            pv=np.concatenate(pvs, axis=0),
            ci=np.concatenate(cis, axis=0),
            nf=np.concatenate(nfs, axis=0),
            pl=np.concatenate(pls, axis=0),
            fs=np.concatenate(fss, axis=0),
            gid=np.concatenate(gids, axis=0),
        )
    tot_rows = sum(len(x) for x in pls)
    print(f"\n=== Twin-World RCT Complete: {len(results)} pairs, {tot_rows:,} rows in {time.perf_counter()-t_start:.1f}s ===")
    by_t: Dict[int, List[dict]] = {}
    for r in results:
        by_t.setdefault(r["actual_turn"], []).append(r)
    summary = {}
    for t in sorted(by_t):
        rs = by_t[t]
        u_pl = float(np.mean([r["pl_upg"] for r in rs]))
        s_pl = float(np.mean([r["pl_stay"] for r in rs]))
        u_p1 = float(np.mean([r["pl_upg"] == 1 for r in rs]))
        s_p1 = float(np.mean([r["pl_stay"] == 1 for r in rs]))
        d_fs = float(np.mean([r["fs_upg"] - r["fs_stay"] for r in rs]))
        summary[str(t)] = {
            "n": len(rs),
            "upg_pl": u_pl,
            "stay_pl": s_pl,
            "ate_dPL": s_pl - u_pl,
            "upg_p1": u_p1,
            "stay_p1": s_p1,
            "ate_dP1": u_p1 - s_p1,
            "ate_dFS": d_fs,
        }
        print(
            f"  Turn {t:2d} (n={len(rs):2d}): UPG={u_pl:.2f} ({u_p1*100:4.1f}% 1st) vs STAY={s_pl:.2f} ({s_p1*100:4.1f}% 1st) | "
            f"ATE: dPL={s_pl-u_pl:+.2f}, dP1={(u_p1-s_p1)*100:+.1f}pp, dFS={d_fs:+.2f}"
        )

    Path(args.out_json).parent.mkdir(parents=True, exist_ok=True)
    with open(args.out_json, "w") as f:
        json.dump({"pairs": results, "summary": summary, "total_rows": tot_rows}, f, indent=2)


if __name__ == "__main__":
    main()
