"""Expert Iteration Self-Play Trainer for `SynergyValueNet` (Set-Transformer Placement Value Model)."""

from __future__ import annotations

import argparse
import math
import multiprocessing as mp
import os
import random
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

import numpy as np
import seaglass
import torch
import torch.nn as nn
import torch.nn.functional as F

from .baseline import GreedyLevelBot, TempoBot
from .evaluator import LeafEvaluator
from .search import ShopEpochBeamBot
from .value_model import (
    DEFAULT_CHECKPOINT_PATH,
    MAX_TOKENS,
    PLACEMENT_UTILITY_WEIGHTS,
    TOKEN_NUM_DIM,
    StateFeaturizer,
    SynergyValueNet,
    get_value_predictor,
)

MAX_STEPS_PER_TURN = 70


def _board_power_metric(obs: seaglass.TavernObservation) -> float:
    stat_sum = sum(max(0, u.attack) + max(0, u.health) for u in obs.board)
    kw_sum = sum(
        (6.0 if u.divine_shield else 0.0)
        + (8.0 if u.venomous else 0.0)
        + (4.0 if u.reborn else 0.0)
        + (5.0 if u.is_golden else 0.0)
        for u in obs.board
    )
    return math.log1p(stat_sum + kw_sum) + 0.5 * obs.tavern_tier + (obs.health + obs.armor) / 30.0


def _worker_init() -> None:
    os.environ["OMP_NUM_THREADS"] = "1"
    os.environ["MKL_NUM_THREADS"] = "1"
    torch.set_num_threads(1)


def _simulate_game_worker(args: Tuple[int, int, Optional[str], int]) -> Dict[str, np.ndarray]:
    """Run one 8-player LobbyGame and featurize trajectories for all 8 seats."""
    game_id, seed, ckpt_path, iter_idx = args
    torch.set_num_threads(1)
    rng = random.Random(seed)
    featurizer = StateFeaturizer("full")

    lobby = seaglass.LobbyGame(players=8, duos=False, seed=seed)

    # Probe active lobby tribes once hero selection finishes; pick diverse seat roles.
    bots: Dict[int, Any] = {}
    seat_configs: List[str] = []
    for p in range(8):
        s_seed = seed + p * 31 + 7
        if iter_idx == 0:
            # Iteration 0: 4 Physics-Explorer Search seats + 2 GreedyLevel + 2 Tempo.
            if p < 4:
                tier_bias = rng.uniform(-0.8, 3.8)
                ev = LeafEvaluator(
                    catalog_name="full",
                    use_physics_rollout=True,
                    use_neural_value=False,
                    explore_tier_bias=tier_bias,
                )
                bots[p] = ShopEpochBeamBot(
                    beam_width=5,
                    max_epoch_depth=5,
                    top_k_combat_evals=2,
                    refresh_samples=1,
                    num_particles=2,
                    seed=s_seed,
                    evaluator=ev,
                )
                bots[p].positioner.max_candidates = 12
                seat_configs.append("phys_explore")
            elif p < 6:
                bots[p] = GreedyLevelBot(seed=s_seed)
                seat_configs.append("greedy")
            else:
                bots[p] = TempoBot(seed=s_seed)
                seat_configs.append("tempo")
        else:
            # Iteration 1+: 5 Neural+Physics Search seats + 1 Physics-Prior + 1 Greedy + 1 Tempo.
            if p < 5:
                tier_bias = 0.0 if p < 3 else rng.uniform(-0.5, 2.5)
                n_w = 0.75 if p < 3 else 0.55
                ev = LeafEvaluator(
                    catalog_name="full",
                    use_physics_rollout=True,
                    use_neural_value=True,
                    neural_weight=n_w,
                    checkpoint_path=ckpt_path,
                    explore_tier_bias=tier_bias,
                )
                bots[p] = ShopEpochBeamBot(
                    beam_width=6,
                    max_epoch_depth=5,
                    top_k_combat_evals=2,
                    refresh_samples=1,
                    num_particles=2,
                    seed=s_seed,
                    evaluator=ev,
                )
                bots[p].positioner.max_candidates = 12
                seat_configs.append("neural_phys")
            elif p == 5:
                ev = LeafEvaluator(
                    catalog_name="full",
                    use_physics_rollout=True,
                    use_neural_value=False,
                )
                bots[p] = ShopEpochBeamBot(
                    beam_width=5,
                    max_epoch_depth=5,
                    top_k_combat_evals=2,
                    refresh_samples=1,
                    num_particles=2,
                    seed=s_seed,
                    evaluator=ev,
                )
                bots[p].positioner.max_candidates = 12
                seat_configs.append("phys_prior")
            elif p == 6:
                bots[p] = GreedyLevelBot(seed=s_seed)
                seat_configs.append("greedy")
            else:
                bots[p] = TempoBot(seed=s_seed)
                seat_configs.append("tempo")

    # Per-player trajectory records: (vocab, slot, pos, num, mask, glob, turn, power_now)
    traj: Dict[int, List[Tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, int, float]]] = {
        p: [] for p in range(8)
    }
    assigned_tribes = False

    while not lobby.done:
        lobby.begin_phase()
        is_turn = lobby.phase == "turn"
        turn_num = lobby.turn

        if is_turn and not assigned_tribes:
            assigned_tribes = True
            active_p = lobby.active_players()
            if active_p:
                ctx0 = lobby.context(active_p[0])
                lobby_tribes = list(ctx0.tribes) if ctx0.tribes else []
                for p, cfg in enumerate(seat_configs):
                    if cfg in ("phys_explore", "neural_phys") and lobby_tribes and p % 2 == 1:
                        bots[p].evaluator.explore_tribe = rng.choice(lobby_tribes)

        for p in list(lobby.active_players()):
            if is_turn and turn_num >= 2:
                # 1. Start-of-turn snapshot (matches `start_turn()` output of 1-turn physics rollouts!).
                obs_s = lobby.observe(p)
                ctx_s = lobby.context(p)
                v_s = np.zeros(MAX_TOKENS, dtype=np.int64)
                sl_s = np.zeros(MAX_TOKENS, dtype=np.int64)
                po_s = np.zeros(MAX_TOKENS, dtype=np.int64)
                nu_s = np.zeros((MAX_TOKENS, TOKEN_NUM_DIM), dtype=np.float32)
                ma_s = np.zeros(MAX_TOKENS, dtype=np.bool_)
                gl_s = np.zeros(featurizer.global_dim, dtype=np.float32)
                featurizer.featurize_into(obs_s, ctx_s, True, v_s, sl_s, po_s, nu_s, ma_s, gl_s)
                traj[p].append((v_s, sl_s, po_s, nu_s, ma_s, gl_s, turn_num, _board_power_metric(obs_s)))

            steps = 0
            recorded_mid = False
            while not lobby.is_done(p) and steps < MAX_STEPS_PER_TURN:
                if is_turn and turn_num >= 3 and steps == 2 and not recorded_mid:
                    recorded_mid = True
                    obs_m = lobby.observe(p)
                    ctx_m = lobby.context(p)
                    v_m = np.zeros(MAX_TOKENS, dtype=np.int64)
                    sl_m = np.zeros(MAX_TOKENS, dtype=np.int64)
                    po_m = np.zeros(MAX_TOKENS, dtype=np.int64)
                    nu_m = np.zeros((MAX_TOKENS, TOKEN_NUM_DIM), dtype=np.float32)
                    ma_m = np.zeros(MAX_TOKENS, dtype=np.bool_)
                    gl_m = np.zeros(featurizer.global_dim, dtype=np.float32)
                    featurizer.featurize_into(obs_m, ctx_m, False, v_m, sl_m, po_m, nu_m, ma_m, gl_m)
                    traj[p].append((v_m, sl_m, po_m, nu_m, ma_m, gl_m, turn_num, _board_power_metric(obs_m)))

                act = bots[p].act_in_lobby(lobby, p)
                if not lobby.is_legal(p, act):
                    lobby.force_end(p)
                    break
                if is_turn and turn_num >= 2 and act.kind == "end_turn":
                    obs_e = lobby.observe(p)
                    ctx_e = lobby.context(p)
                    v_e = np.zeros(MAX_TOKENS, dtype=np.int64)
                    sl_e = np.zeros(MAX_TOKENS, dtype=np.int64)
                    po_e = np.zeros(MAX_TOKENS, dtype=np.int64)
                    nu_e = np.zeros((MAX_TOKENS, TOKEN_NUM_DIM), dtype=np.float32)
                    ma_e = np.zeros(MAX_TOKENS, dtype=np.bool_)
                    gl_e = np.zeros(featurizer.global_dim, dtype=np.float32)
                    featurizer.featurize_into(obs_e, ctx_e, True, v_e, sl_e, po_e, nu_e, ma_e, gl_e)
                    traj[p].append((v_e, sl_e, po_e, nu_e, ma_e, gl_e, turn_num, _board_power_metric(obs_e)))

                lobby.step(p, act)
                steps += 1
            if not lobby.is_done(p):
                lobby.force_end(p)
        lobby.end_phase()

    placements = lobby.placements
    all_v: List[np.ndarray] = []
    all_sl: List[np.ndarray] = []
    all_po: List[np.ndarray] = []
    all_nu: List[np.ndarray] = []
    all_ma: List[np.ndarray] = []
    all_gl: List[np.ndarray] = []
    all_place: List[int] = []
    all_power_future: List[float] = []
    all_weights: List[float] = []
    all_gids: List[int] = []

    for p in range(8):
        if seat_configs[p] in ("greedy", "tempo"):
            # Keep baseline opponent bots in the lobby for strategic diversity,
            # but exclude off-policy baseline trajectories (e.g. suicidal Tier-6 rush)
            # from neural value targets so `tavern_tier` valuation stays unconfounded.
            continue
        pl = placements[p]
        if pl is None:
            continue
        p_label = max(0, min(7, int(pl) - 1))
        p_steps = traj[p]
        n_s = len(p_steps)
        for idx, (v, sl, po, nu, ma, gl, t_num, p_now) in enumerate(p_steps):
            # Look ahead ~2 turns (~4-6 snapshots) for dense warband growth target.
            fut_idx = min(n_s - 1, idx + 4)
            p_fut = p_steps[fut_idx][7] - p_now
            w = min(1.5, 0.40 + 0.10 * t_num)
            all_v.append(v)
            all_sl.append(sl)
            all_po.append(po)
            all_nu.append(nu)
            all_ma.append(ma)
            all_gl.append(gl)
            all_place.append(p_label)
            all_power_future.append(p_fut)
            all_weights.append(w)
            all_gids.append(game_id)

    return {
        "vocab": np.stack(all_v, axis=0),
        "slot": np.stack(all_sl, axis=0),
        "pos": np.stack(all_po, axis=0),
        "num": np.stack(all_nu, axis=0),
        "mask": np.stack(all_ma, axis=0),
        "glob": np.stack(all_gl, axis=0),
        "place": np.asarray(all_place, dtype=np.int64),
        "power_delta": np.asarray(all_power_future, dtype=np.float32),
        "weight": np.asarray(all_weights, dtype=np.float32),
        "game_id": np.asarray(all_gids, dtype=np.int64),
    }


def _spearman_corr(x: np.ndarray, y: np.ndarray) -> float:
    if len(x) < 3:
        return 0.0
    rx = np.argsort(np.argsort(x)).astype(np.float64)
    ry = np.argsort(np.argsort(y)).astype(np.float64)
    rx -= rx.mean()
    ry -= ry.mean()
    denom = math.sqrt(float((rx * rx).sum() * (ry * ry).sum()))
    if denom <= 1e-12:
        return 0.0
    return float((rx * ry).sum() / denom)


def collect_self_play_games(
    num_games: int,
    base_seed: int,
    game_id_offset: int,
    ckpt_path: Optional[str],
    iter_idx: int,
    workers: int,
) -> Dict[str, np.ndarray]:
    t0 = time.perf_counter()
    tasks = [
        (game_id_offset + i, base_seed + i * 137, ckpt_path, iter_idx)
        for i in range(num_games)
    ]
    chunks: List[Dict[str, np.ndarray]] = []
    ctx = mp.get_context("spawn")
    with ctx.Pool(processes=workers, initializer=_worker_init) as pool:
        for done_idx, res in enumerate(pool.imap_unordered(_simulate_game_worker, tasks), 1):
            chunks.append(res)
            if done_idx % max(1, num_games // 4) == 0 or done_idx == num_games:
                elapsed = time.perf_counter() - t0
                n_states = sum(len(c["place"]) for c in chunks)
                print(
                    f"  [Iter {iter_idx}] Simulated {done_idx}/{num_games} games "
                    f"({n_states} states) in {elapsed:.1f}s ({done_idx / max(0.01, elapsed):.1f} games/s)"
                )

    merged: Dict[str, np.ndarray] = {}
    for k in chunks[0].keys():
        merged[k] = np.concatenate([c[k] for c in chunks], axis=0)
    return merged


def train_synergy_net(
    dataset: Dict[str, np.ndarray],
    out_ckpt: Path,
    epochs: int = 16,
    batch_size: int = 512,
    lr: float = 2.5e-3,
    d_model: int = 64,
    n_heads: int = 4,
    n_layers: int = 2,
    warm_ckpt: Optional[Path] = None,
) -> SynergyValueNet:
    torch.set_num_threads(max(1, min(8, (os.cpu_count() or 4) // 2)))
    featurizer = StateFeaturizer("full")
    meta = featurizer.meta
    tag_mat = np.asarray(meta.tag_matrix, dtype=np.float32)

    net = SynergyValueNet(
        vocab_size=meta.vocab_size,
        tag_matrix=tag_mat,
        global_dim=featurizer.global_dim,
        d_model=d_model,
        n_heads=n_heads,
        n_layers=n_layers,
    )
    if warm_ckpt is not None and warm_ckpt.is_file():
        prev = torch.load(str(warm_ckpt), map_location="cpu", weights_only=False)
        net.load_state_dict(prev["state_dict"], strict=True)

    # Split train / val strictly by game_id (90% / 10%) so zero states leak across games!
    unique_games = np.unique(dataset["game_id"])
    rng = np.random.default_rng(2026)
    shuffled_games = unique_games.copy()
    rng.shuffle(shuffled_games)
    n_val_games = max(1, len(shuffled_games) // 10)
    val_game_set = set(shuffled_games[:n_val_games].tolist())
    val_mask = np.isin(dataset["game_id"], list(val_game_set))
    tr_mask = ~val_mask

    def _slice(m: np.ndarray) -> Dict[str, torch.Tensor]:
        return {
            "vocab": torch.from_numpy(dataset["vocab"][m]),
            "slot": torch.from_numpy(dataset["slot"][m]),
            "pos": torch.from_numpy(dataset["pos"][m]),
            "num": torch.from_numpy(dataset["num"][m]),
            "mask": torch.from_numpy(dataset["mask"][m]),
            "glob": torch.from_numpy(dataset["glob"][m]),
            "place": torch.from_numpy(dataset["place"][m]),
            "power": torch.from_numpy(dataset["power_delta"][m]),
            "weight": torch.from_numpy(dataset["weight"][m]),
        }

    tr_data = _slice(tr_mask)
    va_data = _slice(val_mask)
    n_train = len(tr_data["place"])
    n_val = len(va_data["place"])
    print(f"  Training SynergyValueNet on {n_train} states (held-out val: {n_val} states across {n_val_games} games)...")

    optimizer = torch.optim.AdamW(net.parameters(), lr=lr, weight_decay=1e-4)
    scheduler = torch.optim.lr_scheduler.CosineAnnealingLR(optimizer, T_max=epochs, eta_min=lr * 0.08)

    best_val_loss = 1e18
    best_state = None
    place_weights_np = PLACEMENT_UTILITY_WEIGHTS

    for ep in range(1, epochs + 1):
        net.train()
        perm = torch.randperm(n_train)
        tr_loss_sum = 0.0
        n_batches = 0

        for start in range(0, n_train, batch_size):
            idx = perm[start : start + batch_size]
            logits, power_pred = net(
                tr_data["vocab"][idx],
                tr_data["slot"][idx],
                tr_data["pos"][idx],
                tr_data["num"][idx],
                tr_data["mask"][idx],
                tr_data["glob"][idx],
            )
            ce_per = F.cross_entropy(logits, tr_data["place"][idx], reduction="none", label_smoothing=0.03)
            mse_per = F.smooth_l1_loss(power_pred, tr_data["power"][idx], reduction="none")
            w = tr_data["weight"][idx]
            loss = ((ce_per + 0.15 * mse_per) * w).mean()

            optimizer.zero_grad()
            loss.backward()
            nn.utils.clip_grad_norm_(net.parameters(), 2.0)
            optimizer.step()

            tr_loss_sum += float(loss.item())
            n_batches += 1

        scheduler.step()

        # Held-out validation evaluation.
        net.eval()
        with torch.inference_mode():
            v_logits, v_pow = net(
                va_data["vocab"],
                va_data["slot"],
                va_data["pos"],
                va_data["num"],
                va_data["mask"],
                va_data["glob"],
            )
            v_ce = float(F.cross_entropy(v_logits, va_data["place"]).item())
            v_probs = F.softmax(v_logits, dim=-1).numpy()
            v_util = v_probs @ place_weights_np
            true_places = va_data["place"].numpy()
            true_top4 = (true_places <= 3).astype(np.int32)
            pred_top4 = (v_probs[:, :4].sum(axis=1) >= 0.5).astype(np.int32)
            top4_acc = 100.0 * float((true_top4 == pred_top4).mean())
            # Higher utility should correspond to better (smaller) placement index -> negate place.
            rho = _spearman_corr(v_util, -true_places.astype(np.float32))
            _ = v_pow

        if v_ce < best_val_loss:
            best_val_loss = v_ce
            best_state = {k: v.cpu().clone() for k, v in net.state_dict().items()}

        if ep in (1, epochs // 2, epochs) or ep % 4 == 0:
            print(
                f"    Epoch {ep:2d}/{epochs} | train_loss={tr_loss_sum / max(1, n_batches):.4f} "
                f"| val_ce={v_ce:.4f} | val_top4_acc={top4_acc:.1f}% | spearman_rho={rho:.3f}"
            )

    if best_state is not None:
        net.load_state_dict(best_state)

    out_ckpt.parent.mkdir(parents=True, exist_ok=True)
    torch.save(
        {
            "state_dict": net.state_dict(),
            "d_model": d_model,
            "n_heads": n_heads,
            "n_layers": n_layers,
            "vocab_size": meta.vocab_size,
            "global_dim": featurizer.global_dim,
            "val_ce": best_val_loss,
        },
        str(out_ckpt),
    )
    print(f"  -> Saved Set-Transformer checkpoint to {out_ckpt} (best val_ce={best_val_loss:.4f})")
    return net


def main() -> None:
    parser = argparse.ArgumentParser(description="Expert Iteration Trainer for SynergyValueNet")
    parser.add_argument("--iter0-games", type=int, default=160)
    parser.add_argument("--iter1-games", type=int, default=160)
    parser.add_argument("--epochs", type=int, default=16)
    parser.add_argument("--workers", type=int, default=max(1, min(20, (os.cpu_count() or 8) - 2)))
    parser.add_argument("--seed", type=int, default=30000)
    parser.add_argument("--out", type=str, default=str(DEFAULT_CHECKPOINT_PATH))
    args = parser.parse_args()

    out_path = Path(args.out)
    print(f"=== Expert Iteration Round 0: Generating {args.iter0_games} diverse 8-player lobbies ({args.workers} workers) ===")
    ds0 = collect_self_play_games(
        num_games=args.iter0_games,
        base_seed=args.seed,
        game_id_offset=0,
        ckpt_path=None,
        iter_idx=0,
        workers=args.workers,
    )
    train_synergy_net(ds0, out_path, epochs=args.epochs)

    if args.iter1_games > 0:
        print(f"\n=== Expert Iteration Round 1: Generating {args.iter1_games} Neural+Physics lobbies ({args.workers} workers) ===")
        ds1 = collect_self_play_games(
            num_games=args.iter1_games,
            base_seed=args.seed + 500_000,
            game_id_offset=args.iter0_games,
            ckpt_path=str(out_path),
            iter_idx=1,
            workers=args.workers,
        )
        combined = {
            k: np.concatenate([ds0[k], ds1[k]], axis=0) for k in ds0.keys()
        }
        train_synergy_net(
            combined,
            out_path,
            epochs=args.epochs,
            lr=1.5e-3,
            warm_ckpt=out_path,
        )

    predictor = get_value_predictor("full", checkpoint_path=out_path, force_reload=True)
    print("\n=== Top Learned 2-Card Synergy Pairs (Non-Linear Interaction Lift) ===")
    for rank, (lift, name_a, name_b) in enumerate(predictor.top_learned_synergy_pairs(top_k=15), 1):
        print(f"  {rank:2d}. {lift:+6.2f} utility lift | {name_a}  +  {name_b}")


if __name__ == "__main__":
    main()
