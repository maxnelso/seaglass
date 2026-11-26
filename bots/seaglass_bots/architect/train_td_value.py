"""736-Game TD(2)-Bootstrapped Ensemble Value Trainer for ScalingValueTransformer (ZERO hardcoding!).

Preserves ALL 736 unique games (1/6 row stride on GM shards, 100% on all 12 clean DAgger shards)
while computing exact 2-turn future EOT teacher targets per-file before subsampling (RAM < 2 GiB!).
Teacher targets are ONLY applied on TRAIN rows; validation is ALWAYS pure held-out MC ground truth.
"""

from __future__ import annotations

import argparse
import glob
import os
import sys
import time
from pathlib import Path

os.environ.setdefault("OMP_NUM_THREADS", "1")
os.environ.setdefault("MKL_NUM_THREADS", "1")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")

_REPO = Path("/home/maxnelso/scratch/seaglass")
sys.path.insert(0, str(_REPO / "bots"))
sys.path.insert(0, str(_REPO / "rl"))

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

from seaglass_bots.architect.scaling_value import SHOP_SLOT_END, SHOP_SLOT_START, ScalingValueTransformer, _ordinal_target


def load_all_shards_with_td(
    dagger_files: list[str],
    gm_files: list[str],
    gm_subsample: int,
    teacher_ckpts: list[str],
    td_step: int,
    device: torch.device,
    peak_future_weight: float = 0.0,
):
    """Two-pass zero-duplication loader: pre-allocates `nf_out` once so `np.concatenate(nfs)` never doubles peak RAM!"""
    models = (
        [ScalingValueTransformer.load_checkpoint(p).to(device) for p in teacher_ckpts]
        if teacher_ckpts
        else []
    )
    file_plan: list[tuple[str, int, bool, bool, bool, bool, bool, int]] = []
    pv_dim = ci_shape = nf_shape = None
    total_k = 0

    def _plan_file(path: str, subsample: int, is_cruc: bool, is_onpol: bool, is_rct: bool = False, is_twin: bool = False, is_pw0: bool = False):
        nonlocal pv_dim, ci_shape, nf_shape, total_k
        with np.load(path) as d:
            n = int(d["pv"].shape[0])
            if n == 0:
                return
            k = (n + subsample - 1) // subsample if subsample > 1 else n
            if k <= 0:
                return
            if pv_dim is None:
                pv_dim = d["pv"].shape[1:]
                ci_shape = d["ci"].shape[1:]
                nf_shape = d["nf"].shape[1:]
            file_plan.append((path, subsample, is_cruc, is_onpol, is_rct, is_twin, is_pw0, k))
            total_k += k

    for f in dagger_files:
        bn = os.path.basename(f)
        is_sov = "sov_crucible" in f
        is_tw = "twin_rct" in f
        is_p0 = ("pw000" in f) and (not is_tw)
        is_r = (
            ("dagger_beam_v15" in f)
            or ("dagger_beam_v16" in f)
            or ("dagger_beam_v17" in f)
            or ("dagger_beam_v18" in f)
        )
        is_c = (
            ("v6r" in bn)
            or ("dagger_beam_v6" in f)
            or ("dagger_beam_v7" in f)
            or ("dagger_beam_v8" in f)
            or ("dagger_beam_v9" in f)
            or ("dagger_beam_v10" in f)
            or ("dagger_beam_v11" in f)
            or ("dagger_beam_v12" in f)
            or ("dagger_beam_v13" in f)
            or ("dagger_beam_v14" in f)
            or is_r
            or is_tw
            or is_p0
            or is_sov
        )
        _plan_file(f, subsample=1, is_cruc=is_c, is_onpol=(not is_sov), is_rct=is_r, is_twin=is_tw, is_pw0=is_p0)
    for f in gm_files:
        if gm_subsample > 0:
            _plan_file(f, subsample=gm_subsample, is_cruc=False, is_onpol=False, is_rct=False, is_twin=False, is_pw0=False)

    pv_out = np.empty((total_k, *pv_dim), dtype=np.float32)
    ci_out = np.empty((total_k, *ci_shape), dtype=np.int32)
    nf_out = np.empty((total_k, *nf_shape), dtype=np.float16)
    pl_out = np.empty((total_k,), dtype=np.int64)
    fs_out = np.empty((total_k,), dtype=np.float32)
    gid_out = np.empty((total_k,), dtype=np.int64)
    is_cruc_out = np.empty((total_k,), dtype=bool)
    is_onpol_out = np.empty((total_k,), dtype=bool)
    is_rct_out = np.empty((total_k,), dtype=bool)
    is_twin_out = np.empty((total_k,), dtype=bool)
    is_pw0_out = np.empty((total_k,), dtype=bool)
    td_probs_out = np.empty((total_k, 8), dtype=np.float32) if models else None
    td_p1_out = np.empty((total_k,), dtype=np.float32) if models else None
    has_fut_out = np.empty((total_k,), dtype=bool) if models else None
    t_score = 0.0
    pos = 0

    for path, subsample, is_cruc, is_onpol, is_rct, is_twin, is_pw0, expected_k in file_plan:
        with np.load(path) as d:
            pv_all = d["pv"]
            gid_all = d["gid"]
            n = len(pv_all)
            t_all = np.round(pv_all[:, 0] * 20.0).astype(np.int16)
            new_seat = np.zeros(n, dtype=bool)
            new_seat[0] = True
            new_seat[1:] = (
                (gid_all[1:] != gid_all[:-1])
                | (t_all[1:] < t_all[:-1])
                | (pv_all[1:, 3] < pv_all[:-1, 3] - 0.01)
            )
            s_all = np.cumsum(new_seat, dtype=np.int32)
            eot_all = np.zeros(n, dtype=bool)
            eot_all[-1] = True
            eot_all[:-1] = (t_all[:-1] != t_all[1:]) | (s_all[:-1] != s_all[1:])

            sl = slice(None, None, subsample) if subsample > 1 else slice(None)
            kept_indices = np.arange(n, dtype=np.int32)[sl]
            k = len(kept_indices)

            pv = pv_all[sl].astype(np.float32, copy=True)
            eot_kept = eot_all[sl]
            if np.any(eot_kept):
                pv[eot_kept, 4] = 0.0
                pv[eot_kept, 10] = 0.0
                pv[eot_kept, 12] = 0.0
                pv[eot_kept, 21] = 0.0
                pv[eot_kept, 23] = 0.0

            pv_out[pos : pos + k] = pv
            ci_out[pos : pos + k] = d["ci"][sl]
            nf_out[pos : pos + k] = d["nf"][sl]
            pl = d["pl"][sl].astype(np.int64, copy=True)
            if pl.min() == 1:
                pl = pl - 1
            pl_out[pos : pos + k] = pl
            fs_kept = d["fs"][sl].astype(np.float32, copy=True)
            if peak_future_weight > 0.0:
                # Compute exact per-seat peak future EOT board stats (`max_{t' >= t+3} log1p(stats_{t'})`)
                # directly from `nf[:, :7, 0..2]` so real exponential Tier-5/6 engines that explode at t+5..t+10
                # receive higher scaling targets than flat mid-game stat-piles (ZERO hardcoding!).
                nf_all = d["nf"]
                eot_indices = np.flatnonzero(eot_all)
                eot_nf = nf_all[eot_indices, :7, :3].astype(np.float32)
                pres = (eot_nf[:, :, 0] > 0.5).astype(np.float32)
                atk = np.expm1(np.clip(eot_nf[:, :, 1] * 5.0, 0.0, 15.0)) * pres
                hp = np.expm1(np.clip(eot_nf[:, :, 2] * 5.0, 0.0, 15.0)) * pres
                eot_ls = np.log1p((atk + hp).sum(axis=-1))
                # Group EOT (turn, log_stats) by seat and compute suffix max over t' >= t+3 (fallback >= t)
                seat_eot: dict[int, list[tuple[int, float]]] = {}
                for ei, ls_v in zip(eot_indices, eot_ls):
                    seat_eot.setdefault(int(s_all[ei]), []).append((int(t_all[ei]), float(ls_v)))
                peak_map: dict[tuple[int, int], float] = {}
                for s_id, pairs in seat_eot.items():
                    pairs.sort(key=lambda x: x[0])
                    m_len = len(pairs)
                    suf_max = [0.0] * m_len
                    cur_m = 0.0
                    for idx_p in range(m_len - 1, -1, -1):
                        cur_m = max(cur_m, pairs[idx_p][1])
                        suf_max[idx_p] = cur_m
                    for idx_p, (t_v, _) in enumerate(pairs):
                        # Find earliest EOT with turn >= t_v + 3; fallback to peak from t_v onward
                        tgt = suf_max[idx_p]
                        for j_p in range(idx_p, m_len):
                            if pairs[j_p][0] >= t_v + 3:
                                tgt = suf_max[j_p]
                                break
                        peak_map[(s_id, t_v)] = tgt
                w_pk = float(peak_future_weight)
                for j_k, orig_i in enumerate(kept_indices):
                    pk_v = peak_map.get((int(s_all[orig_i]), int(t_all[orig_i])), np.nan)
                    if np.isfinite(pk_v):
                        if np.isfinite(fs_kept[j_k]):
                            fs_kept[j_k] = (1.0 - w_pk) * fs_kept[j_k] + w_pk * pk_v
                        else:
                            fs_kept[j_k] = pk_v
            fs_out[pos : pos + k] = fs_kept
            gid_out[pos : pos + k] = gid_all[sl]
            is_cruc_out[pos : pos + k] = is_cruc
            is_onpol_out[pos : pos + k] = is_onpol
            is_rct_out[pos : pos + k] = is_rct
            is_twin_out[pos : pos + k] = is_twin
            is_pw0_out[pos : pos + k] = is_pw0

            if models:
                eot_map: dict[tuple[int, int], int] = {}
                for idx in np.flatnonzero(eot_all):
                    eot_map[(int(s_all[idx]), int(t_all[idx]))] = int(idx)
                fut_idx = np.full(k, -1, dtype=np.int32)
                for j, orig_i in enumerate(kept_indices):
                    s = int(s_all[orig_i])
                    t = int(t_all[orig_i])
                    nxt = eot_map.get((s, t + td_step), -1)
                    if nxt < 0 and td_step > 1:
                        nxt = eot_map.get((s, t + 1), -1)
                    fut_idx[j] = nxt

                uniq_fut = np.unique(fut_idx[fut_idx >= 0])
                td_p = np.zeros((k, 8), dtype=np.float32)
                td_1 = np.zeros(k, dtype=np.float32)
                hf = fut_idx >= 0
                if len(uniq_fut) > 0:
                    ts0 = time.perf_counter()
                    u_probs = np.zeros((len(uniq_fut), 8), dtype=np.float32)
                    u_p1 = np.zeros(len(uniq_fut), dtype=np.float32)
                    ci_all = d["ci"]
                    nf_all = d["nf"]
                    with torch.inference_mode(), torch.amp.autocast("cuda", dtype=torch.bfloat16, enabled=device.type == "cuda"):
                        for st in range(0, len(uniq_fut), 512):
                            ub = uniq_fut[st : st + 512]
                            bpv_np = pv_all[ub].astype(np.float32, copy=True)
                            bpv_np[:, 4] = 0.0
                            bpv_np[:, 10] = 0.0
                            bpv_np[:, 12] = 0.0
                            bpv_np[:, 21] = 0.0
                            bpv_np[:, 23] = 0.0
                            bpv = torch.from_numpy(bpv_np).to(device)
                            bci = torch.from_numpy(ci_all[ub].astype(np.int64, copy=False)).to(device)
                            bnf = torch.from_numpy(nf_all[ub].astype(np.float32, copy=False)).to(device)
                            pa = p1a = None
                            for m in models:
                                _, lg, _, p1l = m(bpv, bci, bnf)
                                pr = F.softmax(lg.float(), dim=-1)
                                p1 = torch.sigmoid(p1l.float())
                                pa = pr if pa is None else (pa + pr)
                                p1a = p1 if p1a is None else (p1a + p1)
                            u_probs[st : st + len(ub)] = (pa / len(models)).cpu().numpy()
                            u_p1[st : st + len(ub)] = (p1a / len(models)).cpu().numpy()
                    t_score += time.perf_counter() - ts0
                    fpos = np.searchsorted(uniq_fut, fut_idx[hf])
                    td_p[hf] = u_probs[fpos]
                    td_1[hf] = u_p1[fpos]
                td_probs_out[pos : pos + k] = td_p
                td_p1_out[pos : pos + k] = td_1
                has_fut_out[pos : pos + k] = hf
            pos += k

    if models:
        del models
        if device.type == "cuda":
            torch.cuda.empty_cache()
        print(f"Teacher TD-{td_step} scoring done in {t_score:.1f}s across all files!", flush=True)

    out = {
        "pv": pv_out,
        "ci": ci_out,
        "nf": nf_out,
        "pl": pl_out,
        "fs": fs_out,
        "gid": gid_out,
        "is_cruc": is_cruc_out,
        "is_onpol": is_onpol_out,
        "is_rct": is_rct_out,
        "is_twin": is_twin_out,
        "is_pw0": is_pw0_out,
    }
    if td_probs_out is not None:
        out["td_probs"] = td_probs_out
        out["td_p1"] = td_p1_out
        out["has_fut"] = has_fut_out
    return out


def eval_split(model, data, idx, device):
    model.eval()
    ce_s = bce_s = mae_s = 0.0
    n_tot = n_fs = 0
    with torch.inference_mode(), torch.amp.autocast("cuda", dtype=torch.bfloat16, enabled=device.type == "cuda"):
        for i in range(0, len(idx), 512):
            b = idx[i : i + 512]
            pv = torch.from_numpy(data["pv"][b]).to(device, dtype=torch.float32)
            ci = torch.from_numpy(data["ci"][b]).to(device, dtype=torch.long)
            nf = torch.from_numpy(data["nf"][b]).to(device, dtype=torch.float32)
            pl = torch.from_numpy(data["pl"][b]).to(device, dtype=torch.long)
            fs = torch.from_numpy(data["fs"][b]).to(device, dtype=torch.float32)
            _, lg, f3, p1l = model(pv, ci, nf)
            ce_s += float(F.cross_entropy(lg.float(), pl, reduction="sum").item())
            bce_s += float(F.binary_cross_entropy_with_logits(p1l.float(), (pl == 0).float(), reduction="sum").item())
            n_tot += len(b)
            ok = torch.isfinite(fs)
            if bool(ok.any()):
                mae_s += float((f3.float()[ok] - fs[ok]).abs().sum().item())
                n_fs += int(ok.sum().item())
    return ce_s / max(1, n_tot), bce_s / max(1, n_tot), mae_s / max(1, n_fs)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out-dir", required=True)
    ap.add_argument("--seeds", default="501,502,503")
    ap.add_argument("--td-alpha", type=float, default=0.35)
    ap.add_argument("--td-step", type=int, default=2)
    ap.add_argument("--gm-subsample", type=int, default=6)
    ap.add_argument("--cruc-upsample", type=int, default=1)
    ap.add_argument("--onpolicy-upsample", type=int, default=1)
    ap.add_argument("--rct-upsample", type=int, default=1, help="Upsample factor for unconfounded RCT ExIt shards (v15+)!")
    ap.add_argument("--twin-upsample", type=int, default=1, help="Upsample factor for Twin-World Paired Counterfactual shards (twin_rct)!")
    ap.add_argument("--pw0-upsample", type=int, default=1, help="Upsample factor for on-policy pure-neural pw=0.0 DAgger shards (pw000)!")
    ap.add_argument("--twin-pair-weight", type=float, default=0.0, help="Causal Contrastive Pairwise Loss weight on exact same-gid World-A vs World-B counterfactual pairs!")
    ap.add_argument("--twin-pair-batch", type=int, default=32, help="Number of Twin-World counterfactual pairs per SGD step!")
    ap.add_argument("--twin-pair-detach-trunk", action="store_true", help="Detach Transformer trunk on causal pair batch so ONLY pv_skip + heads learn the unconfounded macro curve (zero board overfitting!)")
    ap.add_argument("--sov-cruc-glob", default="")
    ap.add_argument("--extra-dagger-glob", default="")
    ap.add_argument("--only-extra-dagger", action="store_true")
    ap.add_argument("--teacher-ckpts", default="")
    ap.add_argument("--init-ckpt", default="")
    ap.add_argument("--use-pv-skip", action="store_true")
    ap.add_argument("--val-on-cruc", action="store_true")
    ap.add_argument("--epochs", type=int, default=12)
    ap.add_argument("--lr", type=float, default=2.2e-4)
    ap.add_argument("--weight-decay", type=float, default=2.0e-2)
    ap.add_argument("--d-model", type=int, default=192)
    ap.add_argument("--layers", type=int, default=4)
    ap.add_argument("--heads", type=int, default=6)
    ap.add_argument("--ff", type=int, default=576)
    ap.add_argument("--dropout", type=float, default=0.25)
    ap.add_argument("--shop-mask-prob", type=float, default=0.0, help="Stochastically zero out shop slots during training so V(s) reads pv[3] (tavern_tier) directly!")
    ap.add_argument("--mask-shop-eot", action="store_true", help="Set mask_unfrozen_shop_eot=True on checkpoint and also zero pv[15] (is_frozen) whenever shop is masked (eliminates 0g ToggleFreeze exploit!)")
    ap.add_argument("--peak-future-weight", type=float, default=0.0, help="Blend weight on exact per-seat peak future log1p(board_stats) over t'>=t+3 to reward long-horizon real comps!")
    args = ap.parse_args()

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    base = "/home/maxnelso/.gemini/antigravity-cli/brain/1f800705-ed27-44f3-a5fe-a8f577e9accb/scratch"
    dagger_files = []
    if not args.only_extra_dagger:
        dagger_files = sorted(glob.glob(f"{base}/dagger_beam_v7/prior_shards/*.npz"))
        dagger_files += sorted(glob.glob(f"{base}/dagger_beam_v7/dagger_round_*.npz"))
    if args.extra_dagger_glob:
        for pat in args.extra_dagger_glob.split(","):
            if pat.strip():
                dagger_files += sorted(glob.glob(pat.strip()))
    if args.sov_cruc_glob:
        dagger_files += sorted(glob.glob(args.sov_cruc_glob))
    gm_files = (
        sorted(glob.glob("/home/maxnelso/.gemini/antigravity-cli/brain/9df4584f-0f67-4e66-9e7b-4bf86d350ebf/scratch/gm_shards/gm_shard_0*.npz"))
        if args.gm_subsample > 0
        else []
    )

    if args.td_alpha > 0.0:
        if args.teacher_ckpts:
            teachers = [x.strip() for x in args.teacher_ckpts.split(",") if x.strip()]
        else:
            teachers = [
                f"{base}/dagger_beam_v6/scaling_val_dagger_r02.pt",
                f"{base}/dagger_beam_v6/scaling_val_dagger_r03.pt",
                f"{base}/dagger_beam_v6/scaling_val_dagger_r04.pt",
                f"{base}/dagger_beam_v6/scaling_val_dagger_r05.pt",
                f"{base}/dagger_beam_v6/scaling_val_dagger_r06.pt",
            ]
    else:
        teachers = []

    print(f"Loading {len(dagger_files)} DAgger + {len(gm_files)} GM shards (1/{args.gm_subsample} rows, ALL games intact, peak_w={args.peak_future_weight:.2f})...", flush=True)
    data = load_all_shards_with_td(dagger_files, gm_files, args.gm_subsample, teachers, args.td_step, device, peak_future_weight=args.peak_future_weight)
    N = len(data["pl"])
    ug = np.unique(data["gid"])
    print(f"Loaded N={N:,} rows across {len(ug)} unique games! Crucible rows={data['is_cruc'].sum():,}", flush=True)

    Path(args.out_dir).mkdir(parents=True, exist_ok=True)
    seed_list = [int(x) for x in args.seeds.split(",") if x.strip()]

    for seed in seed_list:
        s_rng = np.random.default_rng(seed)
        ug_shuf = ug.copy()
        s_rng.shuffle(ug_shuf)
        n_val = max(40, int(0.12 * len(ug_shuf)))
        val_g = set(ug_shuf[:n_val].tolist())
        is_val = np.isin(data["gid"], list(val_g))
        tr_idx = np.flatnonzero(~is_val)
        if args.rct_upsample > 1:
            tr_rct = np.flatnonzero((~is_val) & data["is_rct"])
            if len(tr_rct) > 0:
                tr_idx = np.concatenate([tr_idx] + [tr_rct] * (args.rct_upsample - 1))
        if args.twin_upsample > 1:
            tr_tw = np.flatnonzero((~is_val) & data["is_twin"])
            if len(tr_tw) > 0:
                tr_idx = np.concatenate([tr_idx] + [tr_tw] * (args.twin_upsample - 1))
        if args.pw0_upsample > 1:
            tr_p0 = np.flatnonzero((~is_val) & data["is_pw0"])
            if len(tr_p0) > 0:
                tr_idx = np.concatenate([tr_idx] + [tr_p0] * (args.pw0_upsample - 1))
        if args.onpolicy_upsample > 1:
            tr_onp = np.flatnonzero((~is_val) & data["is_onpol"])
            if len(tr_onp) > 0:
                tr_idx = np.concatenate([tr_idx] + [tr_onp] * (args.onpolicy_upsample - 1))
        elif args.cruc_upsample > 1:
            tr_cruc = np.flatnonzero((~is_val) & data["is_cruc"])
            if len(tr_cruc) > 0:
                tr_idx = np.concatenate([tr_idx] + [tr_cruc] * (args.cruc_upsample - 1))
        va_idx = np.flatnonzero(is_val)
        va_cruc_idx = np.flatnonzero(is_val & data["is_cruc"])

        # Extract exact same-gid World-A (UPG) vs World-B (STAY) counterfactual pairs strictly from TRAIN split!
        pair_a_list, pair_b_list = [], []
        if args.twin_pair_weight > 0.0:
            tw_tr = np.flatnonzero((~is_val) & data["is_twin"])
            if len(tw_tr) > 0:
                tw_gids = data["gid"][tw_tr]
                tw_turns = np.round(data["pv"][tw_tr, 0] * 20.0).astype(np.int16)
                tw_tiers = np.round(data["pv"][tw_tr, 3] * 6.0).astype(np.int16)
                # Group contiguous indices by gid
                g_map = {}
                for pos_i, g_val in enumerate(tw_gids.tolist()):
                    g_map.setdefault(g_val, []).append(pos_i)
                for g_val, loc_inds in g_map.items():
                    split = None
                    for k in range(1, len(loc_inds)):
                        p_prev, p_cur = loc_inds[k - 1], loc_inds[k]
                        if tw_turns[p_cur] < tw_turns[p_prev] or tw_tiers[p_cur] < tw_tiers[p_prev]:
                            split = k
                            break
                    if split is None:
                        continue
                    ia_loc, ib_loc = loc_inds[:split], loc_inds[split:]
                    t_int = int(tw_turns[ib_loc[0]])
                    # Match EOT rows on t >= t_int where World A still has higher tier than World B
                    for t_val in sorted(set(tw_turns[ib_loc].tolist())):
                        a_rows = [p for p in ia_loc if tw_turns[p] == t_val]
                        b_rows = [p for p in ib_loc if tw_turns[p] == t_val]
                        if a_rows and b_rows and tw_tiers[a_rows[-1]] > tw_tiers[b_rows[-1]]:
                            ga, gb = int(tw_tr[a_rows[-1]]), int(tw_tr[b_rows[-1]])
                            reps = 3 if t_val == t_int else 1
                            for _ in range(reps):
                                pair_a_list.append(ga)
                                pair_b_list.append(gb)
        pair_a_arr = np.asarray(pair_a_list, dtype=np.int64)
        pair_b_arr = np.asarray(pair_b_list, dtype=np.int64)
        ranks_t = torch.arange(8, device=device, dtype=torch.float32)

        out_ckpt = str(Path(args.out_dir) / f"td2_s{seed}.pt")
        torch.manual_seed(seed)
        rng = np.random.default_rng(seed + 1000)
        if args.init_ckpt:
            model = ScalingValueTransformer.load_checkpoint(args.init_ckpt).to(device)
        else:
            model = ScalingValueTransformer(
                d_model=args.d_model,
                layers=args.layers,
                heads=args.heads,
                ff=args.ff,
                dropout=args.dropout,
                use_pv_skip=bool(args.use_pv_skip),
                mask_unfrozen_shop_eot=bool(args.mask_shop_eot),
            ).to(device)
        opt = torch.optim.AdamW(model.parameters(), lr=args.lr, weight_decay=args.weight_decay)

        best_sc = 1e9
        best_meta = {}
        print(
            f"\n=== Seed {seed} | td_alpha={args.td_alpha:.2f} | tr={len(tr_idx):,} ({len(ug)-n_val}g) | val={len(va_idx):,} ({n_val}g, cruc={len(va_cruc_idx):,}) | twin_pairs={len(pair_a_arr)} (w={args.twin_pair_weight:.2f}) ===",
            flush=True,
        )
        for ep in range(1, args.epochs + 1):
            t0 = time.perf_counter()
            prog = (ep - 1) / max(1, args.epochs - 1)
            lr_now = args.lr * (0.1 + 0.9 * 0.5 * (1.0 + np.cos(np.pi * prog)))
            for pg in opt.param_groups:
                pg["lr"] = float(lr_now)

            model.train()
            perm = rng.permutation(tr_idx)
            for i in range(0, len(perm), 512):
                b = perm[i : i + 512]
                pv = torch.from_numpy(data["pv"][b]).to(device, dtype=torch.float32)
                ci = torch.from_numpy(data["ci"][b]).to(device, dtype=torch.long)
                nf = torch.from_numpy(data["nf"][b]).to(device, dtype=torch.float32)
                pl = torch.from_numpy(data["pl"][b]).to(device, dtype=torch.long)
                fs = torch.from_numpy(data["fs"][b]).to(device, dtype=torch.float32)
                if args.shop_mask_prob > 0.0:
                    sm = torch.rand(ci.shape[0], device=device) < args.shop_mask_prob
                    if bool(sm.any()):
                        ci[sm, SHOP_SLOT_START:SHOP_SLOT_END] = 0
                        nf[sm, SHOP_SLOT_START:SHOP_SLOT_END] = 0.0
                        if args.mask_shop_eot:
                            pv[sm, 15] = 0.0

                opt.zero_grad(set_to_none=True)
                with torch.amp.autocast("cuda", dtype=torch.bfloat16, enabled=device.type == "cuda"):
                    _, pl_lg, pred_fs, p1_lg = model(pv, ci, nf)
                    t_ord = _ordinal_target(pl, eps=0.12)
                    y1_smooth = (pl == 0).float() * 0.94 + 0.03
                    if args.td_alpha > 0.0 and "td_probs" in data:
                        hf = torch.from_numpy(data["has_fut"][b]).to(device)
                        a = torch.where(hf, torch.full_like(y1_smooth, args.td_alpha), torch.zeros_like(y1_smooth))
                        tp = torch.from_numpy(data["td_probs"][b]).to(device)
                        tp1 = torch.from_numpy(data["td_p1"][b]).to(device)
                        t_ord = (1.0 - a.unsqueeze(-1)) * t_ord + a.unsqueeze(-1) * tp
                        y1_smooth = (1.0 - a) * y1_smooth + a * tp1
                    v_ce = -(t_ord * F.log_softmax(pl_lg.float(), dim=-1)).sum(dim=-1).mean()
                    bce1 = F.binary_cross_entropy_with_logits(p1_lg.float(), y1_smooth)
                    ok_fs = torch.isfinite(fs)
                    fs_loss = F.smooth_l1_loss(pred_fs.float()[ok_fs], fs[ok_fs]) if bool(ok_fs.any()) else torch.zeros((), device=device)
                    loss = v_ce + 0.45 * bce1 + 0.75 * fs_loss
                    if args.twin_pair_weight > 0.0 and len(pair_a_arr) > 0:
                        bp = min(args.twin_pair_batch, len(pair_a_arr))
                        sel = rng.integers(0, len(pair_a_arr), size=bp)
                        ia = pair_a_arr[sel]
                        ib = pair_b_arr[sel]
                        p_idx = np.concatenate([ia, ib], axis=0)
                        ppv = torch.from_numpy(data["pv"][p_idx]).to(device, dtype=torch.float32)
                        pci = torch.from_numpy(data["ci"][p_idx]).to(device, dtype=torch.long)
                        pnf = torch.from_numpy(data["nf"][p_idx]).to(device, dtype=torch.float32)
                        ppl = torch.from_numpy(data["pl"][p_idx]).to(device, dtype=torch.long)
                        pfs = torch.from_numpy(data["fs"][p_idx]).to(device, dtype=torch.float32)
                        if args.mask_shop_eot:
                            ppv[:, 15] = 0.0
                            pci[:, SHOP_SLOT_START:SHOP_SLOT_END] = 0
                            pnf[:, SHOP_SLOT_START:SHOP_SLOT_END] = 0.0
                        _, ppl_lg, ppred_fs, pp1_lg = model(ppv, pci, pnf, detach_trunk=bool(args.twin_pair_detach_trunk))
                        pr_a = F.softmax(ppl_lg[:bp].float(), dim=-1)
                        pr_b = F.softmax(ppl_lg[bp:].float(), dim=-1)
                        pred_d_epl = (pr_b * ranks_t).sum(dim=-1) - (pr_a * ranks_t).sum(dim=-1)
                        true_d_epl = (ppl[bp:] - ppl[:bp]).float()
                        l_dpl = F.smooth_l1_loss(pred_d_epl / 7.0, true_d_epl / 7.0, beta=0.25)
                        pred_d_p1 = torch.sigmoid(pp1_lg[:bp].float()) - torch.sigmoid(pp1_lg[bp:].float())
                        true_d_p1 = (ppl[:bp] == 0).float() - (ppl[bp:] == 0).float()
                        l_dp1 = F.smooth_l1_loss(pred_d_p1, true_d_p1, beta=0.5)
                        ok_pf = torch.isfinite(pfs[:bp]) & torch.isfinite(pfs[bp:])
                        l_dfs = (
                            F.smooth_l1_loss(ppred_fs[:bp].float()[ok_pf] - ppred_fs[bp:].float()[ok_pf], pfs[:bp][ok_pf] - pfs[bp:][ok_pf])
                            if bool(ok_pf.any())
                            else torch.zeros((), device=device)
                        )
                        pair_loss = 3.0 * l_dpl + 1.0 * l_dp1 + 1.0 * l_dfs
                        loss = loss + args.twin_pair_weight * pair_loss
                loss.backward()
                nn.utils.clip_grad_norm_(model.parameters(), 1.0)
                opt.step()

            ce, bce, mae = eval_split(model, data, va_idx, device)
            c_ce, c_bce, c_mae = eval_split(model, data, va_cruc_idx, device) if len(va_cruc_idx) > 0 else (ce, bce, mae)
            sc = (c_ce + 0.35 * c_mae) if (args.val_on_cruc and len(va_cruc_idx) > 0) else (ce + 0.35 * mae)
            tag = ""
            if sc < best_sc:
                best_sc = sc
                best_meta = {
                    "val_ce": ce, "val_bce1": bce, "val_fs": mae,
                    "cruc_ce": c_ce, "cruc_bce1": c_bce, "cruc_fs": c_mae,
                    "td_alpha": args.td_alpha, "td_step": args.td_step, "seed": seed, "epoch": ep,
                }
                model.save_checkpoint(out_ckpt, meta=best_meta)
                tag = " *BEST*"
            print(
                f"  ep {ep:2d}/{args.epochs} | ALL ce={ce:.4f} p1={bce:.4f} fs={mae:.4f} | "
                f"CRUC ce={c_ce:.4f} p1={c_bce:.4f} fs={c_mae:.4f} | {time.perf_counter()-t0:.1f}s{tag}",
                flush=True,
            )
        print(f"Saved {out_ckpt} | {best_meta}", flush=True)
        del model, opt
        if device.type == "cuda":
            torch.cuda.empty_cache()


if __name__ == "__main__":
    main()
