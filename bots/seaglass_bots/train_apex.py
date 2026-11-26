"""Parallel League Data Collector & RTX GPU Trainer for `ApexValueNet` (`.npz` weights)."""

from __future__ import annotations

import argparse
import math
import multiprocessing as mp
import os
import random
import time
from pathlib import Path
from typing import Any, Dict, List, Tuple

import numpy as np
import seaglass
import torch
import torch.nn.functional as F
from seaglass_rl.device import add_device_arg, default_threads, default_workers, pick_device

from .apex_evaluator import ApexEvaluator
from .apex_model import (
    APEX_MAX_TOKENS,
    APEX_TOKEN_NUM_DIM,
    DEFAULT_APEX_NPZ_PATH,
    ApexStateFeaturizer,
    ApexValueNet,
)
from .apex_search import ApexBot
from .baseline import GreedyLevelBot, TempoBot
from .duos import DuosSequentialRolloutBot
from .search import ExpectimaxMctsBot, ShopEpochBeamBot

ALL_TRIBES = (
    "beast",
    "demon",
    "dragon",
    "elemental",
    "mech",
    "murloc",
    "pirate",
    "quilboar",
    "undead",
    "aberration",
)


def _board_power(obs: seaglass.TavernObservation) -> float:
    from seaglass_rl.env import compute_board_power

    return float(max(0.0, compute_board_power(obs)))


class _RlWrapper:
    def __init__(self, bot: Any) -> None:
        self.bot = bot

    def act_in_lobby(self, lobby: seaglass.LobbyGame, player: int) -> seaglass.TavernAction:
        from seaglass_rl.actions import END_TURN_INDEX, action_from_index
        from seaglass_rl.play import Seat

        seat = Seat(lobby, player)
        idx = self.bot.act(seat)
        act = action_from_index(idx)
        if not seat.is_legal(act):
            act = action_from_index(END_TURN_INDEX)
            if not seat.is_legal(act):
                legal = seat.legal_actions()
                return legal[0] if legal else seaglass.TavernAction.end_turn()
        return act


def _init_collector_worker() -> None:
    torch.set_num_threads(1)


def _collect_single_game(args: Tuple[int, str, str | None]) -> Dict[str, np.ndarray]:
    seed, mode, existing_weights = args
    is_duos = mode == "duos"
    is_1v1 = mode == "1v1"
    n_players = 2 if is_1v1 else 8
    rng = random.Random(seed)
    featurizer = ApexStateFeaturizer("duos" if is_duos else "full")

    from seaglass_rl.bots import HeuristicBot
    from seaglass_rl.gamerbot import NineKGamerBot

    lobby = seaglass.LobbyGame(players=n_players, duos=is_duos, seed=seed)
    bots: List[Any] = []

    if is_1v1:
        roles_1v1 = ["apex", rng.choice(["beam_v2", "mcts", "gamer_tempo", "apex_exp"])]
        rng.shuffle(roles_1v1)
        for p, r1 in enumerate(roles_1v1):
            s_p = seed * 37 + p
            if r1 == "apex":
                bots.append(
                    ApexBot(
                        beam_width=10,
                        max_epoch_depth=7,
                        top_k_combat_evals=3,
                        refresh_samples=2,
                        num_particles=2,
                        seed=s_p,
                        weights_path=existing_weights,
                    )
                )
            elif r1 == "apex_exp":
                ev = ApexEvaluator(
                    weights_path=existing_weights,
                    explore_tribe=rng.choice(ALL_TRIBES),
                    explore_tier_bias=rng.choice([-1.5, 0.0, 2.0]),
                )
                bots.append(
                    ApexBot(
                        beam_width=10,
                        max_epoch_depth=7,
                        top_k_combat_evals=3,
                        refresh_samples=2,
                        num_particles=2,
                        seed=s_p,
                        evaluator=ev,
                    )
                )
            elif r1 == "mcts":
                bots.append(ExpectimaxMctsBot(num_simulations=20, seed=s_p))
            elif r1 == "beam_v2":
                bots.append(
                    ShopEpochBeamBot(
                        beam_width=8,
                        max_epoch_depth=6,
                        top_k_combat_evals=2,
                        refresh_samples=2,
                        num_particles=2,
                        seed=s_p,
                    )
                )
            else:
                bots.append(_RlWrapper(NineKGamerBot(tempo_levels=True)))
    elif is_duos:
        # 2 Apex Duos teams (1 exploratory) vs 1 DuosRollout team vs 1 9kgamerbot-tempo team
        team_roles = ["apex", "apex_exp", "duos_rollout", "gamer_tempo"]
        rng.shuffle(team_roles)
        for p in range(8):
            role = team_roles[lobby.unit_of(p)]
            s_p = seed * 37 + p
            if role == "apex":
                bots.append(
                    ApexBot(
                        catalog_name="duos",
                        beam_width=10,
                        max_epoch_depth=7,
                        top_k_combat_evals=3,
                        refresh_samples=2,
                        num_particles=2,
                        seed=s_p,
                        weights_path=existing_weights,
                    )
                )
            elif role == "apex_exp":
                exp_tr = rng.choice(ALL_TRIBES)
                ev = ApexEvaluator(
                    catalog_name="duos",
                    weights_path=existing_weights,
                    explore_tribe=exp_tr,
                    explore_tier_bias=rng.choice([-1.5, 0.0, 2.5, 4.5]),
                )
                bots.append(
                    ApexBot(
                        catalog_name="duos",
                        beam_width=10,
                        max_epoch_depth=7,
                        top_k_combat_evals=3,
                        refresh_samples=2,
                        num_particles=2,
                        seed=s_p,
                        evaluator=ev,
                    )
                )
            elif role == "duos_rollout":
                bots.append(DuosSequentialRolloutBot(seed=s_p))
            else:
                bots.append(_RlWrapper(NineKGamerBot(tempo_levels=True)))
    else:
        # 8-player Solo diverse league: 3 Apex variants + 1 BeamV2 + 1 MCTS/Gamer + 2 9kGamerBot-Tempo + 1 Baseline
        roles = [
            "apex",
            "apex_exp1",
            "apex_exp2",
            "beam_v2",
            rng.choice(["mcts", "gamer"]),
            "gamer_tempo",
            "gamer_tempo",
            rng.choice(["tempo", "heuristic", "greedy_level"]),
        ]
        rng.shuffle(roles)
        for p, role in enumerate(roles):
            s_p = seed * 37 + p
            if role == "apex":
                bots.append(
                    ApexBot(
                        beam_width=10,
                        max_epoch_depth=7,
                        top_k_combat_evals=3,
                        refresh_samples=2,
                        num_particles=2,
                        seed=s_p,
                        weights_path=existing_weights,
                    )
                )
            elif role.startswith("apex_exp"):
                exp_tr = rng.choice(ALL_TRIBES) if rng.random() < 0.7 else None
                t_bias = rng.choice([-2.0, 0.0, 2.5, 5.0])
                ev = ApexEvaluator(
                    weights_path=existing_weights,
                    explore_tribe=exp_tr,
                    explore_tier_bias=t_bias,
                )
                bots.append(
                    ApexBot(
                        beam_width=10,
                        max_epoch_depth=7,
                        top_k_combat_evals=3,
                        refresh_samples=2,
                        num_particles=2,
                        seed=s_p,
                        evaluator=ev,
                    )
                )
            elif role == "beam_v2":
                bots.append(
                    ShopEpochBeamBot(
                        beam_width=8,
                        max_epoch_depth=6,
                        top_k_combat_evals=2,
                        refresh_samples=2,
                        num_particles=2,
                        seed=s_p,
                    )
                )
            elif role == "mcts":
                bots.append(ExpectimaxMctsBot(num_simulations=20, seed=s_p))
            elif role == "gamer_tempo":
                bots.append(_RlWrapper(NineKGamerBot(tempo_levels=True)))
            elif role == "gamer":
                bots.append(_RlWrapper(NineKGamerBot(tempo_levels=False)))
            elif role == "tempo":
                bots.append(TempoBot(seed=s_p))
            elif role == "greedy_level":
                bots.append(GreedyLevelBot(seed=s_p))
            else:
                bots.append(_RlWrapper(HeuristicBot()))

    # Per-player recorded snapshots: (vocab, slot, pos, num, mask, glob, cur_log_p, turn)
    snaps: List[List[Tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, float, int]]] = [
        [] for _ in range(n_players)
    ]

    def _record_snap(p: int, end_of_turn: bool) -> None:
        obs = lobby.observe(p)
        if obs.turn < 2:
            return
        ctx = lobby.context(p)
        v = np.zeros(APEX_MAX_TOKENS, dtype=np.int32)
        s = np.zeros(APEX_MAX_TOKENS, dtype=np.int8)
        po = np.zeros(APEX_MAX_TOKENS, dtype=np.int8)
        nu = np.zeros((APEX_MAX_TOKENS, APEX_TOKEN_NUM_DIM), dtype=np.float32)
        ma = np.zeros(APEX_MAX_TOKENS, dtype=np.bool_)
        gl = np.zeros(featurizer.global_dim, dtype=np.float32)
        featurizer.featurize_into(obs, ctx, end_of_turn, v, s, po, nu, ma, gl)
        lp = math.log1p(_board_power(obs))
        snaps[p].append((v, s, po, nu.astype(np.float16), ma, gl.astype(np.float16), lp, int(obs.turn)))

    while not lobby.done:
        lobby.begin_phase()
        is_turn = lobby.phase == "turn"
        active = list(lobby.active_players())
        if is_turn:
            # Record start-of-turn (`end_of_turn=False`) snapshot with 50% subsample so both modes are learned!
            for p in active:
                if lobby.turn >= 3 and rng.random() < 0.5:
                    _record_snap(p, end_of_turn=False)

        for p in active:
            steps = 0
            while not lobby.is_done(p) and steps < 85:
                act = bots[p].act_in_lobby(lobby, p)
                if not lobby.is_legal(p, act):
                    lobby.force_end(p)
                    break
                lobby.step(p, act)
                steps += 1
            if not lobby.is_done(p):
                lobby.force_end(p)
            if is_turn:
                _record_snap(p, end_of_turn=True)

        lobby.end_phase()

    placements = [int(pl) for pl in lobby.placements]
    final_lps = [math.log1p(_board_power(lobby.observe(p))) for p in range(n_players)]

    duos_place_map = {1: 0, 2: 2, 3: 5, 4: 7}
    h2h_place_map = {1: 0, 2: 7}

    all_v, all_s, all_po, all_nu, all_ma, all_gl = [], [], [], [], [], []
    all_place, all_cur_lp, all_growth, all_turn = [], [], [], []

    for p in range(n_players):
        u_idx = lobby.unit_of(p)
        raw_pl = placements[u_idx]
        if is_duos:
            pl_bin = duos_place_map.get(raw_pl, 3)
        elif is_1v1:
            pl_bin = h2h_place_map.get(raw_pl, 7)
        else:
            pl_bin = max(0, min(7, raw_pl - 1))
        fin_lp = final_lps[p]
        for v, s, po, nu, ma, gl, cur_lp, trn in snaps[p]:
            all_v.append(v)
            all_s.append(s)
            all_po.append(po)
            all_nu.append(nu)
            all_ma.append(ma)
            all_gl.append(gl)
            all_place.append(pl_bin)
            all_cur_lp.append(cur_lp)
            all_growth.append(fin_lp - cur_lp)
            all_turn.append(trn)

    if not all_v:
        return {}

    return {
        "vocab": np.stack(all_v),
        "slot": np.stack(all_s),
        "pos": np.stack(all_po),
        "num": np.stack(all_nu),
        "mask": np.stack(all_ma),
        "glob": np.stack(all_gl),
        "place": np.asarray(all_place, dtype=np.int64),
        "cur_lp": np.asarray(all_cur_lp, dtype=np.float32),
        "growth": np.asarray(all_growth, dtype=np.float32),
        "turn": np.asarray(all_turn, dtype=np.float32),
        "game_seed": np.full(len(all_v), seed, dtype=np.int64),
    }


def train_apex_value_net(
    num_games: int = 320,
    duos_fraction: float = 0.20,
    workers: int = 14,
    epochs: int = 24,
    batch_size: int = 512,
    lr: float = 1.5e-3,
    base_seed: int = 200_000,
    init_weights: str | None = None,
    out_path: Path = DEFAULT_APEX_NPZ_PATH,
    device: str | None = None,
) -> None:
    t0 = time.perf_counter()
    jobs: List[Tuple[int, str, str | None]] = []
    duos_mod = max(2, int(round(1.0 / max(0.05, duos_fraction))))
    for i in range(num_games):
        if (i % duos_mod) == 0:
            mode = "duos"
        elif (i % 7) == 1:
            mode = "1v1"
        else:
            mode = "solo"
        jobs.append((base_seed + i * 17, mode, init_weights))

    print(
        f"[ApexTrainer] Collecting {num_games} full-lobby games "
        f"(Solo + Duos, workers={workers}, init={init_weights})..."
    )
    saved_env = {k: os.environ.get(k) for k in ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS")}
    os.environ.update({"OMP_NUM_THREADS": "1", "OPENBLAS_NUM_THREADS": "1", "MKL_NUM_THREADS": "1"})
    try:
        ctx_mp = mp.get_context("spawn")
        chunks: List[Dict[str, np.ndarray]] = []
        done_cnt = 0
        with ctx_mp.Pool(processes=workers, initializer=_init_collector_worker) as pool:
            for item in pool.imap_unordered(_collect_single_game, jobs):
                done_cnt += 1
                if item:
                    chunks.append(item)
                if done_cnt % 32 == 0 or done_cnt == num_games:
                    dt = time.perf_counter() - t0
                    n_states = sum(len(c["place"]) for c in chunks)
                    print(
                        f"  Collected {done_cnt}/{num_games} games "
                        f"({n_states:,} states) in {dt:.1f}s ({done_cnt / dt:.2f} games/s)"
                    )
    finally:
        for k, v in saved_env.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v

    vocab = np.concatenate([c["vocab"] for c in chunks], axis=0)
    slot = np.concatenate([c["slot"] for c in chunks], axis=0)
    pos = np.concatenate([c["pos"] for c in chunks], axis=0)
    num = np.concatenate([c["num"] for c in chunks], axis=0)
    mask = np.concatenate([c["mask"] for c in chunks], axis=0)
    glob = np.concatenate([c["glob"] for c in chunks], axis=0)
    place = np.concatenate([c["place"] for c in chunks], axis=0)
    cur_lp = np.concatenate([c["cur_lp"] for c in chunks], axis=0)
    growth = np.concatenate([c["growth"] for c in chunks], axis=0)
    turns = np.concatenate([c["turn"] for c in chunks], axis=0)
    gseeds = np.concatenate([c["game_seed"] for c in chunks], axis=0)

    # Split strictly by game seed (90% train, 10% val)
    uniq_seeds = np.unique(gseeds)
    rng_np = np.random.default_rng(42)
    rng_np.shuffle(uniq_seeds)
    n_val_games = max(1, len(uniq_seeds) // 10)
    val_seed_set = set(uniq_seeds[:n_val_games].tolist())
    is_val = np.array([int(s) in val_seed_set for s in gseeds], dtype=np.bool_)
    tr_idx = np.nonzero(~is_val)[0]
    va_idx = np.nonzero(is_val)[0]

    device = pick_device(device)
    print(
        f"[ApexTrainer] Dataset ready: {len(tr_idx):,} train states, "
        f"{len(va_idx):,} val states | device={device}"
    )

    featurizer = ApexStateFeaturizer("full")
    tag_mat = np.asarray(featurizer.meta.tag_matrix, dtype=np.float32)
    if init_weights and Path(init_weights).is_file():
        model = ApexValueNet.load_npz(init_weights, tag_matrix=tag_mat).to(device)
        model.train()
    else:
        model = ApexValueNet(
            vocab_size=featurizer.meta.vocab_size,
            tag_matrix=tag_mat,
            global_dim=featurizer.global_dim,
            d_model=96,
            n_heads=4,
            n_layers=3,
        ).to(device)

    opt = torch.optim.AdamW(model.parameters(), lr=lr, weight_decay=1e-2)
    sched = torch.optim.lr_scheduler.CosineAnnealingLR(opt, T_max=epochs, eta_min=lr * 0.05)

    best_val_loss = 1e18
    for ep in range(1, epochs + 1):
        model.train()
        perm = rng_np.permutation(tr_idx)
        tr_loss_sum = 0.0
        tr_batches = 0

        for start in range(0, len(perm), batch_size):
            b_idx = perm[start : start + batch_size]
            b_v = torch.from_numpy(vocab[b_idx].astype(np.int64)).to(device)
            b_s = torch.from_numpy(slot[b_idx].astype(np.int64)).to(device)
            b_p = torch.from_numpy(pos[b_idx].astype(np.int64)).to(device)
            b_n = torch.from_numpy(num[b_idx].astype(np.float32)).to(device)
            b_m = torch.from_numpy(mask[b_idx]).to(device)
            b_g = torch.from_numpy(glob[b_idx].astype(np.float32)).to(device)
            y_pl = torch.from_numpy(place[b_idx]).to(device)
            y_lp = torch.from_numpy(cur_lp[b_idx]).to(device)
            y_gr = torch.from_numpy(growth[b_idx]).to(device)
            w_t = torch.from_numpy(
                np.clip(0.55 + 0.45 * (turns[b_idx] / 10.0), 0.55, 1.15).astype(np.float32)
            ).to(device)

            opt.zero_grad(set_to_none=True)
            logits, p_pred, g_pred = model(b_v, b_s, b_p, b_n, b_m, b_g)
            ce = F.cross_entropy(logits, y_pl, reduction="none", label_smoothing=0.03)
            lp_l = F.smooth_l1_loss(p_pred, y_lp, reduction="none")
            gr_l = F.smooth_l1_loss(g_pred, y_gr, reduction="none")
            loss = (w_t * (ce + 0.15 * lp_l + 0.35 * gr_l)).mean()
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), 2.0)
            opt.step()

            tr_loss_sum += float(loss.item())
            tr_batches += 1

        sched.step()

        # Validation pass
        model.eval()
        va_loss_sum = 0.0
        va_top1_correct = 0
        va_total = 0
        with torch.inference_mode():
            for start in range(0, len(va_idx), batch_size):
                b_idx = va_idx[start : start + batch_size]
                b_v = torch.from_numpy(vocab[b_idx].astype(np.int64)).to(device)
                b_s = torch.from_numpy(slot[b_idx].astype(np.int64)).to(device)
                b_p = torch.from_numpy(pos[b_idx].astype(np.int64)).to(device)
                b_n = torch.from_numpy(num[b_idx].astype(np.float32)).to(device)
                b_m = torch.from_numpy(mask[b_idx]).to(device)
                b_g = torch.from_numpy(glob[b_idx].astype(np.float32)).to(device)
                y_pl = torch.from_numpy(place[b_idx]).to(device)
                y_lp = torch.from_numpy(cur_lp[b_idx]).to(device)
                y_gr = torch.from_numpy(growth[b_idx]).to(device)

                logits, p_pred, g_pred = model(b_v, b_s, b_p, b_n, b_m, b_g)
                ce = F.cross_entropy(logits, y_pl, label_smoothing=0.03)
                lp_l = F.smooth_l1_loss(p_pred, y_lp)
                gr_l = F.smooth_l1_loss(g_pred, y_gr)
                v_loss = float((ce + 0.15 * lp_l + 0.35 * gr_l).item())
                va_loss_sum += v_loss * len(b_idx)
                va_top1_correct += int((logits.argmax(dim=-1) == y_pl).sum().item())
                va_total += len(b_idx)

        va_mean = va_loss_sum / max(1, va_total)
        va_acc = 100.0 * va_top1_correct / max(1, va_total)
        marker = ""
        if va_mean < best_val_loss:
            best_val_loss = va_mean
            model.save_npz(out_path, use_fp16=True)
            marker = f" -> saved {out_path.name}"

        if ep == 1 or ep % 4 == 0 or ep == epochs or marker:
            print(
                f"  Epoch {ep:02d}/{epochs:02d} | train_loss={tr_loss_sum / max(1, tr_batches):.4f} "
                f"| val_loss={va_mean:.4f} | val_place_acc={va_acc:.1f}%{marker}"
            )


def main() -> None:
    parser = argparse.ArgumentParser(description="Train ApexValueNet Set-Transformer")
    parser.add_argument("--games", type=int, default=320)
    parser.add_argument(
        "--workers",
        type=int,
        default=default_workers(cap=14),
        help="self-play worker processes (default: $SEAGLASS_WORKERS, else half the cores, at most 14)",
    )
    parser.add_argument("--epochs", type=int, default=24)
    parser.add_argument("--batch-size", type=int, default=512)
    parser.add_argument("--lr", type=float, default=1.5e-3)
    parser.add_argument("--seed", type=int, default=200_000)
    parser.add_argument("--init", type=str, default=None)
    parser.add_argument("--out", type=str, default=str(DEFAULT_APEX_NPZ_PATH))
    add_device_arg(parser)
    args = parser.parse_args()
    torch.set_num_threads(default_threads())

    train_apex_value_net(
        num_games=args.games,
        workers=args.workers,
        epochs=args.epochs,
        batch_size=args.batch_size,
        lr=args.lr,
        base_seed=args.seed,
        init_weights=args.init,
        out_path=Path(args.out),
        device=args.device,
    )


if __name__ == "__main__":
    main()
