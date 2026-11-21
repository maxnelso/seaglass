"""AlphaZero-Physics Iterated Self-Play Trainer for `ZeroValueNet` (`zero_value_v1.npz`).

Runs parallel 8-player Patch 37.0.2 lobbies using `ZeroPhysicsBot` (zero hardcoded card/hero/trinket
tables!), logs Start-of-Turn + End-of-Turn states with exact Rust physics rollout vectors, and trains
a 1st-place-focused Set-Transformer (`ZeroValueNet`) on CUDA (RTX 3070) with:
  - Ordinal-smoothed 8-way Placement Cross-Entropy
  - Explicit 1st-Place Binary BCE Loss (0.75x weight!)
  - 3-Turn Future Board Power Regression (`log1p(stats_{t+3})`)
  - Intra-Lobby Same-Turn Pairwise Ranking Loss (2.5x weight on 1st-place pairs!)
"""

from __future__ import annotations

import argparse
import multiprocessing as mp
import os
import time
from collections import defaultdict
from pathlib import Path
from typing import Any, Dict, List, Sequence, Tuple

os.environ.setdefault("OMP_NUM_THREADS", "1")
os.environ.setdefault("MKL_NUM_THREADS", "1")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")
os.environ.setdefault("NUMEXPR_NUM_THREADS", "1")

import numpy as np
import seaglass
import torch
import torch.nn.functional as F

from .zero_bot import ZeroPhysicsBot, _total_stats
from .zero_model import (
    DEFAULT_ZERO_NPZ_PATH,
    ZERO_PHYSICS_DIM,
    ZeroFeaturizer,
    ZeroValueNet,
)


def _play_zero_game(args_tuple: Tuple[int, str, float, int, int, int, int, str, bool]) -> Dict[str, Any]:
    seed, weights_path, neural_alpha, beam_w, max_d, top_k, ref_s, lobby_type, two_turn = args_tuple
    featurizer = ZeroFeaturizer()
    lobby = seaglass.LobbyGame(players=8, duos=False, seed=seed)

    probe_bot = ZeroPhysicsBot(
        weights_path="",
        beam_width=6,
        max_depth=3,
        top_k_physics=2,
        refresh_samples=1,
        combats_eval=4,
        two_turn_lookahead=False,
        seed=seed * 37 + 11,
    )

    bots: List[Any] = []
    if lobby_type == "mixed":
        from ..sovereign_search import SovereignBot

        for p in range(8):
            if p % 2 == 0:
                bots.append(
                    ZeroPhysicsBot(
                        weights_path=weights_path,
                        beam_width=beam_w,
                        max_depth=max_d,
                        top_k_physics=top_k,
                        refresh_samples=ref_s,
                        combats_eval=6,
                        neural_alpha=neural_alpha,
                        two_turn_lookahead=two_turn,
                        explore_noise=0.9 if (p % 4 == 0) else 0.25,
                        seed=seed * 53 + p * 19 + 7,
                    )
                )
            else:
                bots.append(SovereignBot(beam_width=14, seed=seed * 61 + p * 23 + 3))
    elif lobby_type == "mixed_arch":
        from ..sovereign_search import SovereignBot
        from .hybrid import ArchitectBot

        for p in range(8):
            if p < 4:
                bots.append(
                    ZeroPhysicsBot(
                        weights_path=weights_path,
                        beam_width=beam_w,
                        max_depth=max_d,
                        top_k_physics=top_k,
                        refresh_samples=ref_s,
                        combats_eval=6,
                        neural_alpha=neural_alpha,
                        two_turn_lookahead=two_turn,
                        explore_noise=0.8 if (p % 2 == 0) else 0.25,
                        seed=seed * 53 + p * 19 + 7,
                    )
                )
            elif p < 6:
                bots.append(ArchitectBot(beam_width=14, seed=seed * 67 + p * 29 + 5))
            else:
                bots.append(SovereignBot(beam_width=14, seed=seed * 61 + p * 23 + 3))
    else:
        for p in range(8):
            bots.append(
                ZeroPhysicsBot(
                    weights_path=weights_path,
                    beam_width=beam_w,
                    max_depth=max_d,
                    top_k_physics=top_k,
                    refresh_samples=ref_s,
                    combats_eval=6,
                    neural_alpha=neural_alpha,
                    two_turn_lookahead=two_turn,
                    explore_noise=1.2 if (p % 2 == 0) else 0.35,
                    seed=seed * 53 + p * 19 + 7,
                )
            )

    snaps_by_p: Dict[int, List[Tuple]] = defaultdict(list)
    stats_by_p_turn: Dict[Tuple[int, int], float] = {}

    while not lobby.done:
        lobby.begin_phase()
        is_turn = lobby.phase == "turn"
        if is_turn:
            for p in list(lobby.active_players()):
                o_sot = lobby.observe(p)
                c_sot = lobby.context(p)
                v, z, ps, nu, ma, gl = featurizer.featurize_batch(
                    [o_sot], ctx=c_sot, physics_list=None, end_of_turn=False
                )
                snaps_by_p[p].append(
                    (
                        (v[0].astype(np.int32), z[0].astype(np.int8), ps[0].astype(np.int8), nu[0], ma[0], gl[0]),
                        int(o_sot.turn),
                    )
                )

        for p in list(lobby.active_players()):
            steps = 0
            while not lobby.is_done(p) and steps < 80:
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
                t_now = int(o_eot.turn)
                stats_by_p_turn[(p, t_now)] = float(_total_stats(o_eot))
                if isinstance(bots[p], ZeroPhysicsBot) and bots[p].last_physics_vec is not None:
                    pvec = bots[p].last_physics_vec
                else:
                    # Compute exact Rust physics probe for non-Zero seats (SovereignBot/ArchitectBot)!
                    try:
                        bg = lobby.belief_tavern(p, seed=(seed * 97 + t_now * 13 + p) | 1, extra_units=[])
                        ob, ot, ig, _ = probe_bot._opponent_board_and_tier(o_eot, c_eot)
                        pvec, _ = probe_bot.compute_physics_probe(
                            bg, ob, ot, ig, combats=6, seed=(seed * 31 + t_now * 7 + p) | 1
                        )
                    except Exception:
                        pvec = [0.0] * ZERO_PHYSICS_DIM
                v, z, ps, nu, ma, gl = featurizer.featurize_batch(
                    [o_eot], ctx=c_eot, physics_list=[pvec], end_of_turn=True
                )
                snaps_by_p[p].append(
                    (
                        (v[0].astype(np.int32), z[0].astype(np.int8), ps[0].astype(np.int8), nu[0], ma[0], gl[0]),
                        t_now,
                    )
                )
        lobby.end_phase()

    places = list(lobby.placements)
    v_l, z_l, p_l, n_l, m_l, g_l = [], [], [], [], [], []
    pl_l, fp_l, gid_l, t_l = [], [], [], []
    w_stats = 0
    w_tier = 1
    for p in range(8):
        pl = places[p]
        if pl is None:
            continue
        pl_idx = max(0, min(7, int(pl) - 1))
        if pl_idx == 0:
            wo = lobby.observe(p)
            w_stats = _total_stats(wo)
            w_tier = int(wo.tavern_tier)
        max_t = max((t for (pp, t) in stats_by_p_turn if pp == p), default=1)
        for (v0, z0, p0, n0, m0, g0), t_now in snaps_by_p[p]:
            # 3-turn future board power target (teaches network which boards scale exponentially!)
            t_fut = min(max_t, t_now + 3)
            fut_st = stats_by_p_turn.get((p, t_fut), stats_by_p_turn.get((p, max_t), 0.0))
            v_l.append(v0)
            z_l.append(z0)
            p_l.append(p0)
            n_l.append(n0)
            m_l.append(m0)
            g_l.append(g0)
            pl_l.append(pl_idx)
            fp_l.append(float(np.log1p(max(0.0, fut_st))))
            gid_l.append(seed)
            t_l.append(t_now)

    return {
        "vocab": np.stack(v_l, axis=0),
        "zone": np.stack(z_l, axis=0),
        "pos": np.stack(p_l, axis=0),
        "num": np.stack(n_l, axis=0),
        "mask": np.stack(m_l, axis=0),
        "glob": np.stack(g_l, axis=0),
        "place": np.asarray(pl_l, dtype=np.int64),
        "fut_pow": np.asarray(fp_l, dtype=np.float32),
        "game_id": np.asarray(gid_l, dtype=np.int64),
        "turn": np.asarray(t_l, dtype=np.int16),
        "w_stats": w_stats,
        "w_tier": w_tier,
    }


def generate_zero_dataset(
    out_npz: str,
    num_games: int = 240,
    workers: int = 10,
    base_seed: int = 120000,
    weights_path: str = "",
    neural_alpha: float = 0.72,
    beam_width: int = 10,
    max_depth: int = 5,
    top_k: int = 4,
    refresh_samples: int = 3,
    lobby_type: str = "selfplay",
    two_turn: bool = False,
) -> None:
    t0 = time.perf_counter()
    ctx_mp = mp.get_context("spawn")
    jobs = [
        (base_seed + i * 173, weights_path, neural_alpha, beam_width, max_depth, top_k, refresh_samples, lobby_type, two_turn)
        for i in range(num_games)
    ]
    chunks: List[Dict[str, Any]] = []
    w_stats_all: List[int] = []
    w_tiers_all: List[int] = []

    def _save() -> None:
        Path(out_npz).parent.mkdir(parents=True, exist_ok=True)
        np.savez_compressed(
            out_npz,
            vocab=np.concatenate([c["vocab"] for c in chunks], axis=0),
            zone=np.concatenate([c["zone"] for c in chunks], axis=0),
            pos=np.concatenate([c["pos"] for c in chunks], axis=0),
            num=np.concatenate([c["num"] for c in chunks], axis=0),
            mask=np.concatenate([c["mask"] for c in chunks], axis=0),
            glob=np.concatenate([c["glob"] for c in chunks], axis=0),
            place=np.concatenate([c["place"] for c in chunks], axis=0),
            fut_pow=np.concatenate([c["fut_pow"] for c in chunks], axis=0),
            game_id=np.concatenate([c["game_id"] for c in chunks], axis=0),
            turn=np.concatenate([c["turn"] for c in chunks], axis=0),
        )

    with ctx_mp.Pool(processes=workers, maxtasksperchild=12) as pool:
        for idx, ch in enumerate(pool.imap_unordered(_play_zero_game, jobs, chunksize=1), 1):
            chunks.append(ch)
            w_stats_all.append(ch["w_stats"])
            w_tiers_all.append(ch["w_tier"])
            if idx % 20 == 0 or idx == num_games:
                n_st = sum(len(c["place"]) for c in chunks)
                print(
                    f"[ZeroSelfPlay {idx:3d}/{num_games}] {time.perf_counter()-t0:5.0f}s "
                    f"| states={n_st:6d} | win_stats_med={np.median(w_stats_all):.0f} "
                    f"max={max(w_stats_all)} | win_tier_avg={np.mean(w_tiers_all):.2f}",
                    flush=True,
                )
            if idx % 60 == 0 and idx < num_games:
                _save()

    _save()
    print(f"Saved ZeroSelfPlay dataset ({sum(len(c['place']) for c in chunks)} states) -> {out_npz}", flush=True)


def _build_ordinal_targets(places: torch.Tensor, sigma: float = 0.65) -> torch.Tensor:
    ranks = torch.arange(8, device=places.device, dtype=torch.float32).unsqueeze(0)
    dist2 = (ranks - places.float().unsqueeze(1)) ** 2
    logits = -dist2 / (2.0 * sigma * sigma)
    return F.softmax(logits, dim=-1)


def _build_same_lobby_pairs(
    game_ids: np.ndarray,
    turns: np.ndarray,
    places: np.ndarray,
    indices: np.ndarray,
    max_pairs: int = 24000,
    seed: int = 2029,
) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
    rng = np.random.default_rng(seed)
    buckets: Dict[Tuple[int, int], List[int]] = defaultdict(list)
    for idx in indices:
        buckets[(int(game_ids[idx]), int(turns[idx]))].append(int(idx))
    hi_l, lo_l, mg_l = [], [], []
    for members in buckets.values():
        if len(members) < 2:
            continue
        for i in range(len(members)):
            for j in range(i + 1, len(members)):
                a, b = members[i], members[j]
                pa, pb = int(places[a]), int(places[b])
                if pa == pb:
                    continue
                if pa < pb:
                    hi_l.append(a); lo_l.append(b); mg_l.append(float(pb - pa) * 4.5)
                else:
                    hi_l.append(b); lo_l.append(a); mg_l.append(float(pa - pb) * 4.5)
    if not hi_l:
        return np.zeros(0, dtype=np.int64), np.zeros(0, dtype=np.int64), np.zeros(0, dtype=np.float32)
    hi = np.asarray(hi_l, dtype=np.int64)
    lo = np.asarray(lo_l, dtype=np.int64)
    mg = np.asarray(mg_l, dtype=np.float32)
    if len(hi) > max_pairs:
        sel = rng.choice(len(hi), size=max_pairs, replace=False)
        hi, lo, mg = hi[sel], lo[sel], mg[sel]
    return hi, lo, mg


def train_zero_net(
    npz_paths: Sequence[str],
    out_weights: str = DEFAULT_ZERO_NPZ_PATH,
    warm_start: Optional[str] = None,
    d_model: int = 96,
    n_layers: int = 2,
    epochs: int = 28,
    batch_size: int = 512,
    lr: float = 4.5e-4,
    first_weight: float = 1.0,
    val_first_only: bool = False,
    seed: int = 2029,
) -> None:
    torch.manual_seed(seed)
    np.random.seed(seed)
    rng = np.random.default_rng(seed)

    arrays = defaultdict(list)
    weights_l: List[np.ndarray] = []
    is_primary_l: List[np.ndarray] = []
    ds_i = 0
    for p in npz_paths:
        if not os.path.exists(p):
            continue
        d = np.load(p)
        n_rows = len(d["place"])
        for k in ("vocab", "zone", "pos", "num", "mask", "glob", "place", "fut_pow", "game_id", "turn"):
            arrays[k].append(d[k])
        w_val = first_weight if ds_i == 0 else 1.0
        weights_l.append(np.full(n_rows, w_val, dtype=np.float32))
        is_primary_l.append(np.full(n_rows, ds_i == 0, dtype=bool))
        ds_i += 1

    vocab = np.concatenate(arrays["vocab"], axis=0)
    zone = np.concatenate(arrays["zone"], axis=0)
    pos = np.concatenate(arrays["pos"], axis=0)
    num = np.concatenate(arrays["num"], axis=0)
    mask = np.concatenate(arrays["mask"], axis=0)
    glob = np.concatenate(arrays["glob"], axis=0)
    place = np.concatenate(arrays["place"], axis=0)
    fut_pow = np.concatenate(arrays["fut_pow"], axis=0)
    game_id = np.concatenate(arrays["game_id"], axis=0)
    turn = np.concatenate(arrays["turn"], axis=0)
    sample_w = np.concatenate(weights_l, axis=0)
    is_primary = np.concatenate(is_primary_l, axis=0)

    # Strict game-grouped train/val split (zero leakage!)
    if val_first_only and is_primary.any():
        ug_p = np.unique(game_id[is_primary])
        rng.shuffle(ug_p)
        n_val_g = max(10, int(0.15 * len(ug_p)))
        val_games = set(int(g) for g in ug_p[:n_val_g])
    else:
        ug = np.unique(game_id)
        rng.shuffle(ug)
        n_val_g = max(12, int(0.12 * len(ug)))
        val_games = set(int(g) for g in ug[:n_val_g])
    ug_all = np.unique(game_id)
    is_val = np.asarray([int(g) in val_games for g in game_id], dtype=bool)
    tr_idx = np.where(~is_val)[0]
    va_idx = np.where(is_val)[0]

    dev = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    print(
        f"Training ZeroValueNet (d={d_model}, L={n_layers}, first_w={first_weight}) on {dev} | "
        f"train={len(tr_idx)} val={len(va_idx)} games={len(ug_all)}",
        flush=True,
    )

    net = ZeroValueNet(d_model=d_model, n_heads=4, n_layers=n_layers, d_ff=d_model * 2, dropout=0.22).to(dev)
    if warm_start and os.path.exists(warm_start):
        net.load_npz(warm_start)
        print(f"Warm-started ZeroValueNet from {warm_start}", flush=True)

    opt = torch.optim.AdamW(net.parameters(), lr=lr, weight_decay=2.5e-2)
    sched = torch.optim.lr_scheduler.CosineAnnealingLR(opt, T_max=max(1, epochs), eta_min=lr * 0.05)

    p_hi, p_lo, p_mg = _build_same_lobby_pairs(game_id, turn, place, tr_idx, max_pairs=28000, seed=seed)

    def _eval_val() -> Tuple[float, float, float]:
        net.eval()
        ce_s, bce_s, fp_s, cnt = 0.0, 0.0, 0.0, 0
        with torch.no_grad():
            for s0 in range(0, len(va_idx), batch_size):
                idx = va_idx[s0 : s0 + batch_size]
                logits, fp_p = net(
                    torch.from_numpy(vocab[idx]).to(dev),
                    torch.from_numpy(zone[idx]).to(dev),
                    torch.from_numpy(pos[idx]).to(dev),
                    torch.from_numpy(num[idx]).to(dev),
                    torch.from_numpy(mask[idx]).to(dev),
                    torch.from_numpy(glob[idx]).to(dev),
                )
                plb = torch.from_numpy(place[idx]).to(dev)
                fpb = torch.from_numpy(fut_pow[idx]).to(dev)
                ce = F.cross_entropy(logits, plb)
                probs = F.softmax(logits, dim=-1)
                p1 = probs[:, 0].clamp(1e-5, 1.0 - 1e-5)
                bce = F.binary_cross_entropy(p1, (plb == 0).float())
                fpl = F.smooth_l1_loss(fp_p, fpb)
                n_b = len(idx)
                ce_s += float(ce.item()) * n_b
                bce_s += float(bce.item()) * n_b
                fp_s += float(fpl.item()) * n_b
                cnt += n_b
        return ce_s / max(1, cnt), bce_s / max(1, cnt), fp_s / max(1, cnt)

    best_score = 1e9
    best_ce = 1e9
    best_state = None

    for ep in range(1, epochs + 1):
        net.train()
        order = tr_idx.copy()
        rng.shuffle(order)
        tr_loss = 0.0
        n_steps = 0
        for s0 in range(0, len(order), batch_size):
            idx = order[s0 : s0 + batch_size]
            vb = torch.from_numpy(vocab[idx]).to(dev)
            zb = torch.from_numpy(zone[idx]).to(dev)
            pb = torch.from_numpy(pos[idx]).to(dev)
            nb = torch.from_numpy(num[idx]).to(dev)
            mb = torch.from_numpy(mask[idx]).to(dev)
            gb = torch.from_numpy(glob[idx]).to(dev)
            plb = torch.from_numpy(place[idx]).to(dev)
            fpb = torch.from_numpy(fut_pow[idx]).to(dev)

            # Strong anti-memorization augmentation: random vocab-ID dropout (12%) + continuous jitter!
            drop_v = torch.rand_like(vb.float()) < 0.12
            vb = torch.where(drop_v, torch.zeros_like(vb), vb)
            nb = nb + 0.025 * torch.randn_like(nb)
            gb = gb + 0.025 * torch.randn_like(gb)

            logits, fp_pred = net(vb, zb, pb, nb, mb, gb)
            sw = torch.from_numpy(sample_w[idx]).to(dev)
            sw_norm = sw / sw.mean().clamp(min=1e-4)
            soft_t = _build_ordinal_targets(plb, sigma=0.80)
            loss_ce = (-(soft_t * F.log_softmax(logits, dim=-1)).sum(dim=-1) * sw_norm).mean()
            probs = F.softmax(logits, dim=-1)
            p1 = probs[:, 0].clamp(1e-5, 1.0 - 1e-5)
            loss_1st = (F.binary_cross_entropy(p1, (plb == 0).float(), reduction="none") * sw_norm).mean()
            loss_fp = (F.smooth_l1_loss(fp_pred, fpb, reduction="none") * sw_norm).mean()
            loss = loss_ce + 0.75 * loss_1st + 0.35 * loss_fp

            opt.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(net.parameters(), 1.5)
            opt.step()
            tr_loss += float(loss.item())
            n_steps += 1

        # Intra-lobby pairwise ranking pass (2.5x weight on 1st-place pairs!)
        if len(p_hi) >= 64:
            po = rng.permutation(len(p_hi))
            for s0 in range(0, min(len(po), 8192), 256):
                ip = po[s0 : s0 + 256]
                ih, il = p_hi[ip], p_lo[ip]
                mg = torch.from_numpy(p_mg[ip]).to(dev)
                u_hi = net.expected_utility(
                    torch.from_numpy(vocab[ih]).to(dev),
                    torch.from_numpy(zone[ih]).to(dev),
                    torch.from_numpy(pos[ih]).to(dev),
                    torch.from_numpy(num[ih]).to(dev),
                    torch.from_numpy(mask[ih]).to(dev),
                    torch.from_numpy(glob[ih]).to(dev),
                )
                u_lo = net.expected_utility(
                    torch.from_numpy(vocab[il]).to(dev),
                    torch.from_numpy(zone[il]).to(dev),
                    torch.from_numpy(pos[il]).to(dev),
                    torch.from_numpy(num[il]).to(dev),
                    torch.from_numpy(mask[il]).to(dev),
                    torch.from_numpy(glob[il]).to(dev),
                )
                w_p = torch.where(
                    torch.from_numpy(place[ih] == 0).to(dev),
                    torch.full_like(mg, 2.5),
                    torch.ones_like(mg),
                )
                loss_r = (w_p * F.softplus((mg - (u_hi - u_lo)) * 0.22)).mean()
                opt.zero_grad(set_to_none=True)
                loss_r.backward()
                torch.nn.utils.clip_grad_norm_(net.parameters(), 1.5)
                opt.step()

        sched.step()
        v_ce, v_bce, v_fp = _eval_val()
        v_sc = v_ce + 0.85 * v_bce + 0.15 * v_fp
        if v_sc < best_score:
            best_score = v_sc
            best_ce = v_ce
            best_state = {k: v.detach().cpu().clone() for k, v in net.state_dict().items()}
        if ep % 4 == 0 or ep == epochs or ep == 1:
            print(
                f"  Ep {ep:2d}/{epochs} | tr={tr_loss/max(1,n_steps):.4f} "
                f"| val_ce={v_ce:.4f} | val_1st_bce={v_bce:.4f} | val_fp={v_fp:.4f} (best_ce={best_ce:.4f})",
                flush=True,
            )

    if best_state is not None:
        net.load_state_dict(best_state)
    net.save_npz(out_weights, use_fp16=True)
    print(f"Saved ZeroValueNet -> {out_weights} (best val_ce={best_ce:.4f})", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["generate", "train", "both"], default="both")
    parser.add_argument("--lobby-type", choices=["selfplay", "mixed", "mixed_arch"], default="selfplay")
    parser.add_argument("--games", type=int, default=200)
    parser.add_argument("--workers", type=int, default=10)
    parser.add_argument("--base-seed", type=int, default=120000)
    parser.add_argument("--dataset", type=str, default="/tmp/zero_gen0.npz")
    parser.add_argument("--extra-datasets", type=str, nargs="*", default=[])
    parser.add_argument("--gen-weights", type=str, default="")
    parser.add_argument("--warm-start", type=str, default="")
    parser.add_argument("--out-weights", type=str, default=DEFAULT_ZERO_NPZ_PATH)
    parser.add_argument("--neural-alpha", type=float, default=0.72)
    parser.add_argument("--beam-width", type=int, default=10)
    parser.add_argument("--two-turn", action="store_true")
    parser.add_argument("--d-model", type=int, default=128)
    parser.add_argument("--n-layers", type=int, default=3)
    parser.add_argument("--epochs", type=int, default=28)
    parser.add_argument("--lr", type=float, default=3.0e-4)
    parser.add_argument("--first-weight", type=float, default=1.0)
    parser.add_argument("--val-first-only", action="store_true")
    args = parser.parse_args()

    if args.mode in ("generate", "both"):
        generate_zero_dataset(
            out_npz=args.dataset,
            num_games=args.games,
            workers=args.workers,
            base_seed=args.base_seed,
            weights_path=args.gen_weights,
            neural_alpha=args.neural_alpha,
            beam_width=args.beam_width,
            lobby_type=args.lobby_type,
            two_turn=args.two_turn,
        )
    if args.mode in ("train", "both"):
        all_ds = [args.dataset] + list(args.extra_datasets)
        train_zero_net(
            npz_paths=all_ds,
            out_weights=args.out_weights,
            warm_start=args.warm_start or None,
            d_model=args.d_model,
            n_layers=args.n_layers,
            epochs=args.epochs,
            lr=args.lr,
            first_weight=args.first_weight,
            val_first_only=args.val_first_only,
        )


if __name__ == "__main__":
    main()
