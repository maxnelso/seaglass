"""Train Pure 8-Player FFA `SovereignValueNet` (CE + Intra-Lobby Pairwise Ranking + Growth Loss)."""

from __future__ import annotations

import argparse
import ctypes
import gc
import json
import math
import multiprocessing as mp
import os
import time
from collections import defaultdict
from pathlib import Path
from typing import Any, Dict, List, Sequence, Tuple

import numpy as np
import seaglass
import torch
import torch.nn.functional as F

from .apex_search import ApexBot
from .sovereign_model import (
    DEFAULT_SOVEREIGN_NPZ_PATH,
    SOV_PLACEMENT_WEIGHTS,
    SovereignNumpyPredictor,
    SovereignStateFeaturizer,
    SovereignValueNet,
)
from .sovereign_search import SovereignBot


def _trim_memory() -> None:
    gc.collect()
    try:
        ctypes.CDLL("libc.so.6").malloc_trim(0)
    except Exception:
        pass


def _play_mixed_ffa_game(args_tuple: Tuple[int, str | None]) -> Dict[str, Any]:
    """Play 1 pure 8-player FFA game mixing SovereignBot and ApexBot seats and record all 8 trajectories."""
    from seaglass_rl.env import compute_board_power
    from seaglass_rl.play import BOT_MAX_STEPS_PER_TURN

    seed, weights_path = args_tuple
    featurizer = SovereignStateFeaturizer("full")
    lobby = seaglass.LobbyGame(players=8, duos=False, seed=seed)

    bots: List[Any] = []
    for p in range(8):
        s_p = seed * 53 + p
        if p % 2 == 0:
            bots.append(
                SovereignBot(
                    "full",
                    beam_width=18,
                    max_epoch_depth=9,
                    top_k_combat_evals=5,
                    refresh_samples=4,
                    num_particles=3,
                    seed=s_p,
                    weights_path=weights_path,
                )
            )
        else:
            bots.append(
                ApexBot(
                    "full",
                    beam_width=16,
                    max_epoch_depth=8,
                    top_k_combat_evals=4,
                    refresh_samples=3,
                    num_particles=3,
                    seed=s_p,
                )
            )

    snaps_by_p: Dict[int, List[Tuple]] = defaultdict(list)

    while not lobby.done:
        lobby.begin_phase()
        is_turn = lobby.phase == "turn"
        if is_turn:
            for p in list(lobby.active_players()):
                o_sot = lobby.observe(p)
                c_sot = lobby.context(p)
                pw_sot = float(max(0.0, compute_board_power(o_sot)))
                v, s, ps, nu, ma, gl = featurizer.featurize_batch([o_sot], c_sot, False)
                snaps_by_p[p].append(
                    (
                        (
                            v[0].astype(np.int32),
                            s[0].astype(np.int8),
                            ps[0].astype(np.int8),
                            nu[0],
                            ma[0],
                            gl[0],
                        ),
                        pw_sot,
                        int(o_sot.turn),
                    )
                )

        for p in list(lobby.active_players()):
            steps = 0
            while not lobby.is_done(p) and steps < BOT_MAX_STEPS_PER_TURN:
                act = bots[p].act_in_lobby(lobby, p)
                if not lobby.is_legal(p, act):
                    lobby.force_end(p)
                    break
                lobby.step(p, act)
                steps += 1
            if not lobby.is_done(p):
                lobby.force_end(p)

            if is_turn:
                o_eot = lobby.observe(p)
                c_eot = lobby.context(p)
                pw_eot = float(max(0.0, compute_board_power(o_eot)))
                v, s, ps, nu, ma, gl = featurizer.featurize_batch([o_eot], c_eot, True)
                snaps_by_p[p].append(
                    (
                        (
                            v[0].astype(np.int32),
                            s[0].astype(np.int8),
                            ps[0].astype(np.int8),
                            nu[0],
                            ma[0],
                            gl[0],
                        ),
                        pw_eot,
                        int(o_eot.turn),
                    )
                )

        lobby.end_phase()

    placements = [int(pl) for pl in lobby.placements]
    sov_places = [placements[p] for p in range(0, 8, 2)]
    apex_places = [placements[p] for p in range(1, 8, 2)]

    final_powers = [
        float(max(0.0, compute_board_power(lobby.observe(p)))) for p in range(8)
    ]

    v_list, s_list, p_list, n_list, m_list, g_list = [], [], [], [], [], []
    place_list, growth_list, game_id_list, turn_list = [], [], [], []

    for p in range(8):
        pl_zero = placements[p] - 1
        f_log = math.log1p(final_powers[p])
        for (v, s, pos_a, nu, ma, gl), cur_pw, t_num in snaps_by_p[p]:
            v_list.append(v)
            s_list.append(s)
            p_list.append(pos_a)
            n_list.append(nu)
            m_list.append(ma)
            g_list.append(gl)
            place_list.append(pl_zero)
            growth_list.append(f_log - math.log1p(cur_pw))
            game_id_list.append(seed)
            turn_list.append(t_num)

    res = {
        "vocab": np.stack(v_list),
        "slot": np.stack(s_list),
        "pos": np.stack(p_list),
        "num": np.stack(n_list),
        "mask": np.stack(m_list),
        "glob": np.stack(g_list),
        "place": np.asarray(place_list, dtype=np.int64),
        "growth": np.asarray(growth_list, dtype=np.float32),
        "game_id": np.asarray(game_id_list, dtype=np.int64),
        "turn": np.asarray(turn_list, dtype=np.int16),
        "sov_places": sov_places,
        "apex_places": apex_places,
    }
    del bots
    del lobby
    _trim_memory()
    return res


def collect_mixed_dataset(
    num_games: int,
    workers: int,
    base_seed: int,
    out_npz: str,
    weights_path: str | None = None,
) -> None:
    os.environ.update(
        {
            "OMP_NUM_THREADS": "1",
            "OPENBLAS_NUM_THREADS": "1",
            "MKL_NUM_THREADS": "1",
            "MALLOC_ARENA_MAX": "2",
        }
    )
    t0 = time.perf_counter()
    ctx_mp = mp.get_context("spawn")
    jobs = [(base_seed + i, weights_path) for i in range(num_games)]
    chunks = []
    sov_all: List[int] = []
    apex_all: List[int] = []

    with ctx_mp.Pool(processes=workers, maxtasksperchild=3) as pool:
        for idx, ch in enumerate(pool.imap_unordered(_play_mixed_ffa_game, jobs, chunksize=1), 1):
            chunks.append(ch)
            sov_all.extend(ch["sov_places"])
            apex_all.extend(ch["apex_places"])
            if idx % 8 == 0 or idx == num_games:
                print(
                    f"[MixedSelfPlay {idx}/{num_games}] elapsed={time.perf_counter()-t0:.1f}s "
                    f"| Sov AvgPl={np.mean(sov_all):.2f} vs Apex AvgPl={np.mean(apex_all):.2f}",
                    flush=True,
                )

    np.savez_compressed(
        out_npz,
        vocab=np.concatenate([c["vocab"] for c in chunks], axis=0),
        slot=np.concatenate([c["slot"] for c in chunks], axis=0),
        pos=np.concatenate([c["pos"] for c in chunks], axis=0),
        num=np.concatenate([c["num"] for c in chunks], axis=0),
        mask=np.concatenate([c["mask"] for c in chunks], axis=0),
        glob=np.concatenate([c["glob"] for c in chunks], axis=0),
        place=np.concatenate([c["place"] for c in chunks], axis=0),
        growth=np.concatenate([c["growth"] for c in chunks], axis=0),
        game_id=np.concatenate([c["game_id"] for c in chunks], axis=0),
        turn=np.concatenate([c["turn"] for c in chunks], axis=0),
    )
    print(f"Saved mixed 8p-FFA dataset ({sum(len(c['place']) for c in chunks)} states) -> {out_npz}", flush=True)


def _build_ordinal_soft_targets(places: np.ndarray, sigma: float = 0.80) -> np.ndarray:
    """Ordinal-aware soft 8-way placement targets so 1st vs 2nd is closer than 1st vs 8th."""
    grid = np.arange(8, dtype=np.float32)[None, :]  # (1, 8)
    diff = grid - places[:, None].astype(np.float32)
    logits = -0.5 * (diff / sigma) ** 2
    exp_l = np.exp(logits - logits.max(axis=-1, keepdims=True))
    soft = exp_l / exp_l.sum(axis=-1, keepdims=True)
    # Blend 72% hard + 28% ordinal Gaussian to prevent overconfident early-turn logits
    hard = np.eye(8, dtype=np.float32)[places]
    return 0.72 * hard + 0.28 * soft


def _augment_train_inputs(
    v_b: torch.Tensor,
    g_b: torch.Tensor,
) -> Tuple[torch.Tensor, torch.Tensor]:
    """
    Prevent trajectory-ID memorization (`(hero_id, 5-tribe vector) -> game placement`)
    so the Set-Transformer learns generalizable warband, trinket, economy & tempo valuation!
    """
    bsz = v_b.shape[0]
    dev = v_b.device
    v_aug = v_b.clone()
    g_aug = g_b.clone()

    # 1. Stochastically mask Hero token (`slot 0`) on 45% of samples
    drop_hero = torch.rand(bsz, device=dev) < 0.45
    v_aug[drop_hero, 0] = 0

    # 2. Stochastically mask or perturb 5-tribe lobby fingerprint (`glob[:, 38:48]`) on 55% of samples
    r_tr = torch.rand(bsz, device=dev)
    zero_tr = r_tr < 0.35
    g_aug[zero_tr, 38:48] = 0.0
    noisy_tr = (r_tr >= 0.35) & (r_tr < 0.55)
    if noisy_tr.any():
        flip_mask = (torch.rand(( int(noisy_tr.sum().item()), 10), device=dev) < 0.20).float()
        g_aug[noisy_tr, 38:48] = torch.abs(g_aug[noisy_tr, 38:48] - flip_mask)

    # 3. Mild Gaussian jitter on continuous scalar globals (HP, gold, relative stats)
    cont_cols = [2, 3, 4, 12, 13, 20, 26, 29, 30]
    g_aug[:, cont_cols] = g_aug[:, cont_cols] + 0.018 * torch.randn(
        (bsz, len(cont_cols)), device=dev
    )
    return v_aug, g_aug


def _build_same_lobby_pairs(
    game_ids: np.ndarray,
    turns: np.ndarray,
    eot_flags: np.ndarray,
    places: np.ndarray,
    indices: np.ndarray,
    rng: np.random.Generator,
    max_pairs_per_bucket: int = 10,
) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
    """
    Build intra-lobby same-turn `(idx_better, idx_worse, margin)` pairs!
    Teaches `SovereignValueNet` to rank competing warbands inside the EXACT same 5-tribe lobby!
    """
    buckets: Dict[Tuple[int, int, int], List[int]] = defaultdict(list)
    for loc_i in indices:
        g = int(game_ids[loc_i])
        t = int(turns[loc_i])
        if t < 2:
            continue
        e = int(eot_flags[loc_i] > 0.5)
        buckets[(g, t, e)].append(int(loc_i))

    i_better: List[int] = []
    i_worse: List[int] = []
    margins: List[float] = []

    for items in buckets.values():
        if len(items) < 2:
            continue
        cand_pairs: List[Tuple[int, int, float]] = []
        for a in range(len(items)):
            ia = items[a]
            pa = int(places[ia])
            for b in range(a + 1, len(items)):
                ib = items[b]
                pb = int(places[ib])
                if pa == pb:
                    continue
                if pa < pb:
                    hi, lo = ia, ib
                    phi, plo = pa, pb
                else:
                    hi, lo = ib, ia
                    phi, plo = pb, pa
                m = 0.16 * float(SOV_PLACEMENT_WEIGHTS[phi] - SOV_PLACEMENT_WEIGHTS[plo])
                cand_pairs.append((hi, lo, min(8.0, m)))
        if not cand_pairs:
            continue
        if len(cand_pairs) > max_pairs_per_bucket:
            pick_idx = rng.choice(len(cand_pairs), size=max_pairs_per_bucket, replace=False)
            cand_pairs = [cand_pairs[k] for k in pick_idx]
        for hi, lo, m in cand_pairs:
            i_better.append(hi)
            i_worse.append(lo)
            margins.append(m)

    return (
        np.asarray(i_better, dtype=np.int64),
        np.asarray(i_worse, dtype=np.int64),
        np.asarray(margins, dtype=np.float32),
    )


def train_model(
    npz_paths: Sequence[str],
    out_weights: str,
    epochs: int = 28,
    batch_size: int = 256,
    lr: float = 4.5e-4,
    seed: int = 2029,
) -> Dict[str, float]:
    torch.manual_seed(seed)
    np.random.seed(seed)
    rng = np.random.default_rng(seed)

    parts = [np.load(p, allow_pickle=False) for p in npz_paths]
    vocab = np.concatenate([d["vocab"] for d in parts], axis=0).astype(np.int64)
    slot = np.concatenate([d["slot"] for d in parts], axis=0).astype(np.int64)
    pos = np.concatenate([d["pos"] for d in parts], axis=0).astype(np.int64)
    num = np.concatenate([d["num"] for d in parts], axis=0).astype(np.float32)
    mask = np.concatenate([d["mask"] for d in parts], axis=0).astype(np.bool_)
    glob = np.concatenate([d["glob"] for d in parts], axis=0).astype(np.float32)
    place = np.concatenate([d["place"] for d in parts], axis=0).astype(np.int64)
    growth = np.concatenate([d["growth"] for d in parts], axis=0).astype(np.float32)
    game_id = np.concatenate([d["game_id"] for d in parts], axis=0).astype(np.int64)
    turn = np.concatenate([d["turn"] for d in parts], axis=0).astype(np.int16)

    # Strip unfrozen `slot == 6` shop tokens so cached datasets match `obs.is_frozen`-only featurizer!
    unfrozen_shop = (slot == 6) & (glob[:, 17:18] < 0.5)
    if np.any(unfrozen_shop):
        vocab[unfrozen_shop] = 0
        slot[unfrozen_shop] = 0
        pos[unfrozen_shop] = 0
        num[unfrozen_shop] = 0.0
        mask[unfrozen_shop] = False

    unique_games = np.unique(game_id)
    rng.shuffle(unique_games)
    n_val_games = max(6, int(round(0.14 * len(unique_games))))
    val_game_set = set(unique_games[:n_val_games].tolist())

    is_val = np.array([int(g) in val_game_set for g in game_id], dtype=np.bool_)
    train_idx = np.nonzero(~is_val)[0]
    val_idx = np.nonzero(is_val)[0]

    print(
        f"[TrainSovereign] Total states={len(place)} across {len(unique_games)} pure 8p-FFA games "
        f"(train={len(train_idx)}, val={len(val_idx)})",
        flush=True,
    )

    eot_flags = glob[:, 18]
    val_better, val_worse, _val_margins = _build_same_lobby_pairs(
        game_id, turn, eot_flags, place, val_idx, rng, max_pairs_per_bucket=28
    )

    featurizer = SovereignStateFeaturizer("full")
    tag_mat = np.asarray(featurizer.meta.tag_matrix, dtype=np.float32)
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    print(f"[TrainSovereign] Training on device={device}", flush=True)

    net = SovereignValueNet(
        vocab_size=featurizer.meta.vocab_size,
        tag_matrix=tag_mat,
        global_dim=featurizer.global_dim,
        d_model=96,
        n_heads=4,
        n_layers=3,
    ).to(device)

    vocab_t = torch.from_numpy(vocab).to(device)
    slot_t = torch.from_numpy(slot).to(device)
    pos_t = torch.from_numpy(pos).to(device)
    num_t = torch.from_numpy(num).to(device)
    mask_t = torch.from_numpy(mask).to(device)
    glob_t = torch.from_numpy(glob).to(device)
    soft_targets_t = torch.from_numpy(_build_ordinal_soft_targets(place)).to(device)
    growth_t = torch.from_numpy(growth).to(device)
    turn_weights_np = np.clip(0.25 + 0.15 * turn.astype(np.float32), 0.40, 1.15)
    turn_w_t = torch.from_numpy(turn_weights_np).to(device)

    decay_params = [p for n, p in net.named_parameters() if p.ndim >= 2 and "norm" not in n]
    nodecay_params = [p for n, p in net.named_parameters() if p.ndim < 2 or "norm" in n]
    opt = torch.optim.AdamW(
        [
            {"params": decay_params, "weight_decay": 1.8e-2},
            {"params": nodecay_params, "weight_decay": 0.0},
        ],
        lr=lr,
    )
    sched = torch.optim.lr_scheduler.CosineAnnealingLR(opt, T_max=epochs, eta_min=lr * 0.05)

    best_score = -1e9
    best_metrics: Dict[str, float] = {}

    for ep in range(1, epochs + 1):
        net.train()
        perm_tr = rng.permutation(train_idx)
        ce_sum = 0.0
        rk_sum = 0.0
        gr_sum = 0.0
        n_batches = 0

        # Build fresh intra-lobby ranking pairs each epoch
        tr_better, tr_worse, tr_margins = _build_same_lobby_pairs(
            game_id, turn, eot_flags, place, train_idx, rng, max_pairs_per_bucket=12
        )
        pair_perm = rng.permutation(len(tr_better)) if len(tr_better) > 0 else np.zeros((0,), dtype=np.int64)
        pair_cursor = 0

        for start in range(0, len(perm_tr), batch_size):
            b_idx = perm_tr[start : start + batch_size]
            b_t = torch.from_numpy(b_idx).to(device)

            v_in, g_in = _augment_train_inputs(vocab_t[b_t], glob_t[b_t])
            logits, g_pred = net(
                v_in,
                slot_t[b_t],
                pos_t[b_t],
                num_t[b_t],
                mask_t[b_t],
                g_in,
            )
            log_probs = F.log_softmax(logits, dim=-1)
            per_sample_ce = -(soft_targets_t[b_t] * log_probs).sum(dim=-1)
            loss_ce = (per_sample_ce * turn_w_t[b_t]).mean()
            loss_gr = F.huber_loss(g_pred, growth_t[b_t], delta=1.2)

            loss_rk = torch.tensor(0.0, device=device)
            if len(pair_perm) >= 64:
                if pair_cursor + 128 > len(pair_perm):
                    pair_perm = rng.permutation(len(tr_better))
                    pair_cursor = 0
                p_slice = pair_perm[pair_cursor : pair_cursor + 128]
                pair_cursor += 128
                ib_t = torch.from_numpy(tr_better[p_slice]).to(device)
                iw_t = torch.from_numpy(tr_worse[p_slice]).to(device)
                mg_t = torch.from_numpy(tr_margins[p_slice]).to(device)

                vb_in, gb_in = _augment_train_inputs(vocab_t[ib_t], glob_t[ib_t])
                vw_in, gw_in = _augment_train_inputs(vocab_t[iw_t], glob_t[iw_t])
                u_hi = net.expected_utility(
                    vb_in, slot_t[ib_t], pos_t[ib_t], num_t[ib_t], mask_t[ib_t], gb_in
                )
                u_lo = net.expected_utility(
                    vw_in, slot_t[iw_t], pos_t[iw_t], num_t[iw_t], mask_t[iw_t], gw_in
                )
                loss_rk = F.relu(mg_t - (u_hi - u_lo)).mean()

            loss = loss_ce + 0.28 * loss_rk + 0.25 * loss_gr
            opt.zero_grad()
            loss.backward()
            torch.nn.utils.clip_grad_norm_(net.parameters(), 2.0)
            opt.step()

            ce_sum += float(loss_ce.item())
            rk_sum += float(loss_rk.item())
            gr_sum += float(loss_gr.item())
            n_batches += 1

        sched.step()

        # Validation
        net.eval()
        with torch.inference_mode():
            v_t = torch.from_numpy(val_idx).to(device)
            v_logits, v_gr = net(
                vocab_t[v_t], slot_t[v_t], pos_t[v_t], num_t[v_t], mask_t[v_t], glob_t[v_t]
            )
            v_ce = float(-(soft_targets_t[v_t] * F.log_softmax(v_logits, dim=-1)).sum(dim=-1).mean().item())
            v_gr_loss = float(F.huber_loss(v_gr, growth_t[v_t], delta=1.2).item())

            pair_acc = 0.5
            if len(val_better) > 0:
                vb_t = torch.from_numpy(val_better).to(device)
                vw_t = torch.from_numpy(val_worse).to(device)
                u_vb = net.expected_utility(
                    vocab_t[vb_t], slot_t[vb_t], pos_t[vb_t], num_t[vb_t], mask_t[vb_t], glob_t[vb_t]
                )
                u_vw = net.expected_utility(
                    vocab_t[vw_t], slot_t[vw_t], pos_t[vw_t], num_t[vw_t], mask_t[vw_t], glob_t[vw_t]
                )
                pair_acc = float((u_vb > u_vw).float().mean().item())

        sel_score = pair_acc - 0.28 * v_ce - 0.05 * v_gr_loss
        if ep % 4 == 0 or ep == 1 or ep == epochs or sel_score > best_score:
            print(
                f"  Epoch {ep:02d}/{epochs} | tr_ce={ce_sum/max(1,n_batches):.3f} "
                f"tr_rk={rk_sum/max(1,n_batches):.3f} | val_ce={v_ce:.3f} "
                f"val_pair_acc={100.0*pair_acc:.2f}% {'*' if sel_score > best_score else ''}",
                flush=True,
            )
        if sel_score > best_score:
            best_score = sel_score
            best_metrics = {
                "epoch": float(ep),
                "val_ce": round(v_ce, 4),
                "val_growth_huber": round(v_gr_loss, 4),
                "val_intra_lobby_pair_acc": round(100.0 * pair_acc, 2),
                "train_states": float(len(train_idx)),
                "val_states": float(len(val_idx)),
            }
            net.save_npz(out_weights, use_fp16=True)

    # Calibrate `place_head` temperature `T*` on held-out validation games for sharp probability scaling!
    cal_pt = SovereignValueNet.load_npz(out_weights, tag_matrix=tag_mat).to(device)
    cal_pt.eval()
    with torch.inference_mode():
        v_t = torch.from_numpy(val_idx).to(device)
        cal_logits, _ = cal_pt(
            vocab_t[v_t], slot_t[v_t], pos_t[v_t], num_t[v_t], mask_t[v_t], glob_t[v_t]
        )
        best_T = 1.0
        best_cal_ce = 1e9
        for cand_T in np.linspace(0.75, 2.40, 67):
            c_ce = float(
                -(soft_targets_t[v_t] * F.log_softmax(cal_logits / float(cand_T), dim=-1))
                .sum(dim=-1)
                .mean()
                .item()
            )
            if c_ce < best_cal_ce:
                best_cal_ce = c_ce
                best_T = float(cand_T)
        cal_pt.place_head.weight.data.div_(best_T)
        cal_pt.place_head.bias.data.div_(best_T)
        cal_pt.save_npz(out_weights, use_fp16=True)
        best_metrics["calibrated_temp"] = round(best_T, 3)
        best_metrics["calibrated_val_ce"] = round(best_cal_ce, 4)

    # Verify pure-NumPy BLAS `SovereignNumpyPredictor` numerical parity with PyTorch!
    reloaded_pt = SovereignValueNet.load_npz(out_weights, tag_matrix=tag_mat)
    np_pred = SovereignNumpyPredictor("full", weights_path=out_weights)
    sample_n = min(64, len(val_idx))
    s_idx = val_idx[:sample_n]
    with torch.inference_mode():
        pt_u = reloaded_pt.expected_utility(
            torch.from_numpy(vocab[s_idx]),
            torch.from_numpy(slot[s_idx]),
            torch.from_numpy(pos[s_idx]),
            torch.from_numpy(num[s_idx]),
            torch.from_numpy(mask[s_idx]),
            torch.from_numpy(glob[s_idx]),
        ).numpy()
    np_u = np_pred.forward_arrays(
        vocab[s_idx], slot[s_idx], pos[s_idx], num[s_idx], mask[s_idx], glob[s_idx]
    )
    max_diff = float(np.max(np.abs(pt_u - np_u)))
    best_metrics["numpy_pytorch_max_abs_diff"] = round(max_diff, 6)
    print(
        f"[TrainSovereign] Saved {out_weights} | Best={json.dumps(best_metrics)}",
        flush=True,
    )
    assert max_diff < 5e-3, f"NumPy vs PyTorch mismatch: {max_diff}"
    return best_metrics


def main() -> None:
    parser = argparse.ArgumentParser(description="Train Pure 8-Player FFA SovereignValueNet")
    parser.add_argument("--data-npz", nargs="*", default=[], help="Existing .npz datasets")
    parser.add_argument("--collect-games", type=int, default=0)
    parser.add_argument("--collect-out", type=str, default=None)
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--seed", type=int, default=1_400_000)
    parser.add_argument("--epochs", type=int, default=28)
    parser.add_argument("--out-weights", type=str, default=str(DEFAULT_SOVEREIGN_NPZ_PATH))
    args = parser.parse_args()

    all_npz = list(args.data_npz)
    if args.collect_games > 0:
        c_out = args.collect_out or "/tmp/sovereign_mixed_ffa.npz"
        w_init = args.out_weights if Path(args.out_weights).is_file() else None
        collect_mixed_dataset(
            num_games=args.collect_games,
            workers=args.workers,
            base_seed=args.seed,
            out_npz=c_out,
            weights_path=w_init,
        )
        all_npz.append(c_out)

    if not all_npz:
        raise ValueError("Provide at least one --data-npz or --collect-games > 0")

    train_model(
        npz_paths=all_npz,
        out_weights=args.out_weights,
        epochs=args.epochs,
        seed=args.seed,
    )


if __name__ == "__main__":
    main()
