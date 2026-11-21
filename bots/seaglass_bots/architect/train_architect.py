"""Patch 37.0.2 GPU Self-Play Trainer for `ArchitectValueNet` (`architect_value_v1.npz`).

Generates pure Patch 37.0.2 (post-Weighted-Gauntlet removal, full Aberration trinkets)
8-player FFA trajectories mixing `ArchitectBot` and `SovereignBot` seats, then trains
a 1st-place-sharp Set-Transformer on CUDA (RTX 3070) with:
  - Ordinal-smoothed 8-way Placement Cross-Entropy
  - Extra 1st-Place Binary BCE Loss (explicitly separating 1st-place exponential caps from 3rd/4th tempo!)
  - Intra-Lobby Same-Turn Pairwise Ranking Loss (weighted 2x on 1st-place pairs!)
  - 1-Turn Board Power Growth Regression Loss
  - Anti-memorization augmentation (Hero / 5-tribe / scalar jitter)
"""

from __future__ import annotations

import argparse
import json
import multiprocessing as mp
import os
import time
from collections import defaultdict
from pathlib import Path
from typing import Any, Dict, List, Sequence, Tuple

os.environ.setdefault("OMP_NUM_THREADS", "1")
os.environ.setdefault("MKL_NUM_THREADS", "1")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")

import numpy as np
import seaglass
import torch
import torch.nn.functional as F

from ..sovereign_model import (
    SovereignStateFeaturizer,
    SovereignValueNet,
)
from ..sovereign_search import SovereignBot
from .hybrid import DEFAULT_ARCHITECT_NPZ_PATH, ArchitectBot

# 1st-place-sharp placement utility targets (separates lobby winners from safe Top-4 boards!)
ARCHITECT_PLACE_WEIGHTS = np.array(
    [45.0, 18.0, 6.0, -2.0, -10.0, -18.0, -26.0, -34.0], dtype=np.float32
)


def _play_patch37_game(args_tuple: Tuple[int, str | None]) -> Dict[str, Any]:
    from seaglass_rl.env import compute_board_power

    seed, weights_path = args_tuple
    featurizer = SovereignStateFeaturizer("full")
    lobby = seaglass.LobbyGame(players=8, duos=False, seed=seed)

    bots: List[Any] = []
    for p in range(8):
        s_p = seed * 59 + p * 17 + 3
        if p % 2 == 0:
            ab = ArchitectBot(
                "full",
                beam_width=10,
                max_epoch_depth=6,
                top_k_combat_evals=3,
                refresh_samples=3,
                num_particles=2,
                seed=s_p,
                weights_path=weights_path,
            )
            ab._enable_two_turn_lookahead = False
            bots.append(ab)
        else:
            bots.append(
                SovereignBot(
                    "full",
                    beam_width=10,
                    max_epoch_depth=6,
                    top_k_combat_evals=3,
                    refresh_samples=3,
                    num_particles=2,
                    seed=s_p,
                )
            )

    snaps_by_p: Dict[int, List[Tuple]] = defaultdict(list)
    heroes_by_p: Dict[int, int] = {}
    trinkets_by_p: Dict[int, List[int]] = defaultdict(list)

    while not lobby.done:
        lobby.begin_phase()
        is_turn = lobby.phase == "turn"
        if is_turn:
            for p in list(lobby.active_players()):
                o_sot = lobby.observe(p)
                c_sot = lobby.context(p)
                if p not in heroes_by_p and o_sot.hero_id:
                    heroes_by_p[p] = int(o_sot.hero_id)
                pw_sot = float(max(0.0, compute_board_power(o_sot)))
                v, s, ps, nu, ma, gl = featurizer.featurize_batch([o_sot], c_sot, False)
                snaps_by_p[p].append(
                    (
                        (v[0].astype(np.int32), s[0].astype(np.int8), ps[0].astype(np.int8), nu[0], ma[0], gl[0]),
                        pw_sot,
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
                trinkets_by_p[p] = [int(t) for t in o_eot.trinkets]
                pw_eot = float(max(0.0, compute_board_power(o_eot)))
                v, s, ps, nu, ma, gl = featurizer.featurize_batch([o_eot], c_eot, True)
                snaps_by_p[p].append(
                    (
                        (v[0].astype(np.int32), s[0].astype(np.int8), ps[0].astype(np.int8), nu[0], ma[0], gl[0]),
                        pw_eot,
                        int(o_eot.turn),
                    )
                )
        lobby.end_phase()

    places = list(lobby.placements)
    v_list, s_list, p_list, n_list, m_list, g_list = [], [], [], [], [], []
    pl_list, gr_list, gid_list, turn_list = [], [], [], []

    for p in range(8):
        pl = places[p]
        if pl is None:
            continue
        pl_idx = max(0, min(7, int(pl) - 1))
        traj = snaps_by_p[p]
        for idx, (feats, pw, trn) in enumerate(traj):
            next_pw = traj[min(len(traj) - 1, idx + 2)][1]
            growth = float(np.clip(np.log1p(next_pw) - np.log1p(pw), -2.5, 2.5))
            v, s, ps, nu, ma, gl = feats
            v_list.append(v)
            s_list.append(s)
            p_list.append(ps)
            n_list.append(nu)
            m_list.append(ma)
            g_list.append(gl)
            pl_list.append(pl_idx)
            gr_list.append(growth)
            gid_list.append(seed)
            turn_list.append(trn)

    return {
        "vocab": np.stack(v_list, axis=0),
        "slot": np.stack(s_list, axis=0),
        "pos": np.stack(p_list, axis=0),
        "num": np.stack(n_list, axis=0),
        "mask": np.stack(m_list, axis=0),
        "glob": np.stack(g_list, axis=0),
        "place": np.asarray(pl_list, dtype=np.int8),
        "growth": np.asarray(gr_list, dtype=np.float32),
        "game_id": np.asarray(gid_list, dtype=np.int64),
        "turn": np.asarray(turn_list, dtype=np.int16),
        "arch_places": [places[p] for p in range(0, 8, 2) if places[p] is not None],
        "sov_places": [places[p] for p in range(1, 8, 2) if places[p] is not None],
        "hero_stats": [(heroes_by_p.get(p, 0), places[p]) for p in range(8) if places[p] is not None],
        "trinket_stats": [(trinkets_by_p.get(p, []), places[p]) for p in range(8) if places[p] is not None],
    }


def generate_dataset(
    out_npz: str,
    num_games: int = 80,
    workers: int = 6,
    base_seed: int = 70000,
    weights_path: str | None = None,
) -> None:
    t0 = time.perf_counter()
    ctx_mp = mp.get_context("spawn")
    jobs = [(base_seed + i * 131, weights_path) for i in range(num_games)]
    chunks = []
    arch_all: List[int] = []
    sov_all: List[int] = []

    def _save_snapshot() -> None:
        Path(out_npz).parent.mkdir(parents=True, exist_ok=True)
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
        meta_out = out_npz.replace(".npz", "_meta.json")
        hero_stats = [hs for c in chunks for hs in c["hero_stats"]]
        trinket_stats = [ts for c in chunks for ts in c["trinket_stats"]]
        with open(meta_out, "w") as f:
            json.dump({"hero_stats": hero_stats, "trinket_stats": trinket_stats}, f)

    with ctx_mp.Pool(processes=workers, maxtasksperchild=6) as pool:
        for idx, ch in enumerate(pool.imap_unordered(_play_patch37_game, jobs, chunksize=1), 1):
            chunks.append(ch)
            arch_all.extend(ch["arch_places"])
            sov_all.extend(ch["sov_places"])
            if idx % 4 == 0 or idx == num_games:
                a_1st = 100.0 * sum(p == 1 for p in arch_all) / max(1, len(arch_all))
                s_1st = 100.0 * sum(p == 1 for p in sov_all) / max(1, len(sov_all))
                print(
                    f"[Patch37 SelfPlay {idx}/{num_games}] {time.perf_counter()-t0:.0f}s "
                    f"| Arch Avg={np.mean(arch_all):.2f} (1st={a_1st:.1f}%) "
                    f"vs Sov Avg={np.mean(sov_all):.2f} (1st={s_1st:.1f}%)",
                    flush=True,
                )
            if idx % 16 == 0 and idx < num_games:
                _save_snapshot()

    _save_snapshot()
    print(f"Saved Patch 37.0.2 dataset ({sum(len(c['place']) for c in chunks)} states) -> {out_npz}", flush=True)


def train_architect_net(
    npz_paths: Sequence[str],
    out_weights: str = DEFAULT_ARCHITECT_NPZ_PATH,
    epochs: int = 32,
    batch_size: int = 256,
    lr: float = 4.5e-4,
    seed: int = 2029,
) -> None:
    from ..train_sovereign import _augment_train_inputs, _build_ordinal_soft_targets, _build_same_lobby_pairs

    torch.manual_seed(seed)
    np.random.seed(seed)
    rng = np.random.default_rng(seed)

    parts = [np.load(p, allow_pickle=False) for p in npz_paths]
    vocab = np.concatenate([d["vocab"] for d in parts], axis=0).astype(np.int64)
    slot = np.concatenate([d["slot"] for d in parts], axis=0).astype(np.int64)
    pos = np.concatenate([d["pos"] for d in parts], axis=0).astype(np.int64)
    num = np.concatenate([d["num"] for d in parts], axis=0).astype(np.float32)
    mask = np.concatenate([d["mask"] for d in parts], axis=0).astype(bool)
    glob = np.concatenate([d["glob"] for d in parts], axis=0).astype(np.float32)
    place = np.concatenate([d["place"] for d in parts], axis=0).astype(np.int64)
    growth = np.concatenate([d["growth"] for d in parts], axis=0).astype(np.float32)
    game_id = np.concatenate([d["game_id"] for d in parts], axis=0).astype(np.int64)
    turn = np.concatenate([d["turn"] for d in parts], axis=0).astype(np.int64)

    # Split cleanly by game_id (85% train / 15% val) to prevent leakage!
    uniq_games = np.unique(game_id)
    rng.shuffle(uniq_games)
    n_val_g = max(1, int(0.15 * len(uniq_games)))
    val_games = set(uniq_games[:n_val_g].tolist())
    val_mask = np.asarray([int(g) in val_games for g in game_id], dtype=bool)
    train_idx = np.nonzero(~val_mask)[0]
    val_idx = np.nonzero(val_mask)[0]

    featurizer = SovereignStateFeaturizer("full")
    dev = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    print(f"Training ArchitectValueNet on {dev} | train={len(train_idx)} val={len(val_idx)} games={len(uniq_games)}")

    tag_mat = np.asarray(featurizer.meta.tag_matrix, dtype=np.float32)
    net = SovereignValueNet(
        vocab_size=featurizer.meta.vocab_size,
        tag_matrix=tag_mat,
        global_dim=glob.shape[1],
        d_model=96,
        n_heads=4,
        n_layers=3,
    ).to(dev)

    # Warm-start from sovereign_value_v1.npz if available for fast convergence + zero forgetting!
    sov_init = Path(out_weights).parent / "sovereign_value_v1.npz"
    if sov_init.exists():
        loaded = SovereignValueNet.load_npz(sov_init, tag_mat)
        net.load_state_dict(loaded.state_dict(), strict=True)
        print(f"Warm-started weights from {sov_init}")

    # Install 1st-place-sharp placement weights!
    w_t = torch.from_numpy(ARCHITECT_PLACE_WEIGHTS).to(dev)
    net.place_weights.copy_(w_t)

    soft_targets = _build_ordinal_soft_targets(place, sigma=0.75)
    eot_flags = glob[:, 1]
    p_hi, p_lo, p_m = _build_same_lobby_pairs(game_id, turn, eot_flags, place, train_idx, rng, max_pairs_per_bucket=12)

    opt = torch.optim.AdamW(net.parameters(), lr=lr, weight_decay=1.0e-3)
    sched = torch.optim.lr_scheduler.CosineAnnealingLR(opt, T_max=epochs, eta_min=lr * 0.05)

    def _eval_val() -> Tuple[float, float]:
        net.eval()
        v_ce = 0.0
        v_first_bce = 0.0
        v_n = 0
        with torch.no_grad():
            for s0 in range(0, len(val_idx), 512):
                b = val_idx[s0 : s0 + 512]
                logits, _ = net(
                    torch.from_numpy(vocab[b]).to(dev),
                    torch.from_numpy(slot[b]).to(dev),
                    torch.from_numpy(pos[b]).to(dev),
                    torch.from_numpy(num[b]).to(dev),
                    torch.from_numpy(mask[b]).to(dev),
                    torch.from_numpy(glob[b]).to(dev),
                )
                plb = torch.from_numpy(place[b]).to(dev)
                v_ce += float(F.cross_entropy(logits, plb, reduction="sum").item())
                pf = F.softmax(logits, dim=-1)[:, 0].clamp(1e-5, 1.0 - 1e-5)
                v_first_bce += float(F.binary_cross_entropy(pf, (plb == 0).float(), reduction="sum").item())
                v_n += len(b)
        return v_ce / max(1, v_n), v_first_bce / max(1, v_n)

    base_ce, base_f1 = _eval_val()
    print(f"  Epoch  0 (warm-start baseline) | val_ce={base_ce:.4f} | val_1st_bce={base_f1:.4f}", flush=True)
    best_val_score = base_ce + 0.65 * base_f1
    best_val_loss = base_ce
    best_state = {k: v.detach().cpu().clone() for k, v in net.state_dict().items()}

    for ep in range(1, epochs + 1):
        net.train()
        perm = rng.permutation(train_idx)
        ce_sum = 0.0
        n_steps = 0
        for s0 in range(0, len(perm), batch_size):
            b = perm[s0 : s0 + batch_size]
            if len(b) < 16:
                continue
            vb = torch.from_numpy(vocab[b]).to(dev)
            sb = torch.from_numpy(slot[b]).to(dev)
            pb = torch.from_numpy(pos[b]).to(dev)
            nb = torch.from_numpy(num[b]).to(dev)
            mb = torch.from_numpy(mask[b]).to(dev)
            gb = torch.from_numpy(glob[b]).to(dev)
            tb = torch.from_numpy(soft_targets[b]).to(dev)
            plb = torch.from_numpy(place[b]).to(dev)
            grb = torch.from_numpy(growth[b]).to(dev)

            vb_a, gb_a = _augment_train_inputs(vb, gb)
            logits, gr_pred = net(vb_a, sb, pb, nb, mb, gb_a)
            log_p = F.log_softmax(logits, dim=-1)
            loss_ce = -(tb * log_p).sum(dim=-1).mean()

            # Explicit 1st-place binary BCE loss (forces sharp discrimination of true 1st-place comps!)
            probs = F.softmax(logits, dim=-1)
            p_first = probs[:, 0].clamp(1e-5, 1.0 - 1e-5)
            is_first = (plb == 0).float()
            loss_first = F.binary_cross_entropy(p_first, is_first)

            loss_gr = F.smooth_l1_loss(gr_pred, grb)
            loss = loss_ce + 0.45 * loss_first + 0.35 * loss_gr

            opt.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(net.parameters(), 1.5)
            opt.step()
            ce_sum += float(loss.item())
            n_steps += 1

        # Intra-lobby pairwise ranking step (up-weights pairs involving the 1st-place winner!)
        if len(p_hi) >= 32:
            p_order = rng.permutation(len(p_hi))
            for s0 in range(0, min(len(p_order), 4096), 128):
                idx_p = p_order[s0 : s0 + 128]
                ih = p_hi[idx_p]
                il = p_lo[idx_p]
                mg = torch.from_numpy(p_m[idx_p]).to(dev)
                u_hi = net.expected_utility(
                    torch.from_numpy(vocab[ih]).to(dev),
                    torch.from_numpy(slot[ih]).to(dev),
                    torch.from_numpy(pos[ih]).to(dev),
                    torch.from_numpy(num[ih]).to(dev),
                    torch.from_numpy(mask[ih]).to(dev),
                    torch.from_numpy(glob[ih]).to(dev),
                )
                u_lo = net.expected_utility(
                    torch.from_numpy(vocab[il]).to(dev),
                    torch.from_numpy(slot[il]).to(dev),
                    torch.from_numpy(pos[il]).to(dev),
                    torch.from_numpy(num[il]).to(dev),
                    torch.from_numpy(mask[il]).to(dev),
                    torch.from_numpy(glob[il]).to(dev),
                )
                # 2x weight on pairs where the winner actually placed 1st!
                w_pair = torch.where(
                    torch.from_numpy(place[ih] == 0).to(dev),
                    torch.full_like(mg, 2.0),
                    torch.ones_like(mg),
                )
                loss_rank = (w_pair * F.softplus((mg - (u_hi - u_lo)) * 0.25)).mean()
                opt.zero_grad(set_to_none=True)
                loss_rank.backward()
                torch.nn.utils.clip_grad_norm_(net.parameters(), 1.5)
                opt.step()

        sched.step()

        # Validation
        val_loss, val_f1 = _eval_val()
        val_score = val_loss + 0.65 * val_f1
        if val_score < best_val_score:
            best_val_score = val_score
            best_val_loss = val_loss
            best_state = {k: v.detach().cpu().clone() for k, v in net.state_dict().items()}
        if ep % 2 == 0 or ep == epochs:
            print(
                f"  Epoch {ep:2d}/{epochs} | train_loss={ce_sum/max(1,n_steps):.4f} "
                f"| val_ce={val_loss:.4f} | val_1st_bce={val_f1:.4f} (best_ce={best_val_loss:.4f})",
                flush=True,
            )

    if best_state is not None:
        net.load_state_dict(best_state)
    net.save_npz(out_weights, use_fp16=True)
    print(f"Saved ArchitectValueNet -> {out_weights} (best val_ce={best_val_loss:.4f})", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["generate", "train", "both"], default="both")
    parser.add_argument("--games", type=int, default=64)
    parser.add_argument("--workers", type=int, default=6)
    parser.add_argument("--base-seed", type=int, default=70000)
    parser.add_argument("--dataset", type=str, default="/tmp/architect_p37_ds.npz")
    parser.add_argument("--extra-datasets", type=str, nargs="*", default=[])
    parser.add_argument("--out-weights", type=str, default=DEFAULT_ARCHITECT_NPZ_PATH)
    parser.add_argument("--epochs", type=int, default=14)
    parser.add_argument("--lr", type=float, default=6.0e-5)
    args = parser.parse_args()

    if args.mode in ("generate", "both"):
        generate_dataset(
            out_npz=args.dataset,
            num_games=args.games,
            workers=args.workers,
            base_seed=args.base_seed,
        )
    if args.mode in ("train", "both"):
        all_ds = [args.dataset] + list(args.extra_datasets)
        train_architect_net(
            npz_paths=all_ds,
            out_weights=args.out_weights,
            epochs=args.epochs,
            lr=args.lr,
        )


if __name__ == "__main__":
    main()
