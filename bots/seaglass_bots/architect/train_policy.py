"""Architecture B: GPU-Accelerated Advantage-Weighted Expert Distillation + 1st-Place PPO
for `EntityTransformer` (`seaglass_rl.transformer`).

Zero hardcoded rules: learns an end-to-end 330-action Entity-Transformer Policy + Value network
over all 101 card slots + 278-dim static card tags & rules-text embeddings!

Pipeline:
  1. `--mode collect`: Collects high-skill 8-player trajectories from `ArchitectBot` /
     `ZeroPhysicsBot` / `SovereignBot` lobbies in parallel.
  2. `--mode awbc`: Trains `EntityTransformer` on CUDA (RTX 3070) using 1st-Place
     Advantage-Weighted Cross-Entropy (weights 1st-place winners 5x over losers!) + value MSE.
  3. `--mode ppo`: Runs GPU-accelerated 1st-Place-Focused PPO in 8-player lobbies against
     frozen snapshots + `9kgamerbot-tempo` (and optional `sovereign` anchor seats)!
"""

from __future__ import annotations

import argparse
import multiprocessing as mp
import os
import random
import sys
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple

os.environ.setdefault("OMP_NUM_THREADS", "1")
os.environ.setdefault("MKL_NUM_THREADS", "1")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")
os.environ.setdefault("NUMEXPR_NUM_THREADS", "1")

# Ensure `rl/` is importable cleanly
_RL_DIR = str(Path(__file__).resolve().parents[3] / "rl")
if _RL_DIR not in sys.path:
    sys.path.insert(0, _RL_DIR)

import numpy as np
import seaglass
import torch
import torch.nn as nn
import torch.nn.functional as F

from seaglass_rl import checkpoint as rl_ckpt
from seaglass_rl.device import add_device_arg, default_threads, default_workers, empty_cache, pick_device
from seaglass_rl.actions import (
    ACTION_KIND_IDS,
    ACTION_KINDS,
    ACTION_SPACE_SIZE,
    ACTION_TO_INDEX,
    ACTIONS,
    END_TURN_INDEX,
    action_mask,
)
from seaglass_rl.features import NUM_SLOTS, PLAYER_DIM, SLOT_NUM_DIM, encode_observation
from seaglass_rl.play import GameSpec, LEARNER, LobbyConfig, PlayOptions, Player, Seat
from seaglass_rl.ppo import build_batch, gae
from seaglass_rl.transformer import EntityTransformer


# Sharp 1st-place tournament rewards (emphasizes winning 1st over settling for 3rd/4th!)
FIRST_PLACE_REWARDS = np.array(
    [2.20, 0.55, 0.05, -0.25, -0.45, -0.65, -0.82, -1.00], dtype=np.float32
)
# Advantage-Weighted BC weights by final placement (1..8)
AWBC_PLACE_WEIGHTS = np.array(
    [5.00, 2.20, 1.00, 0.45, 0.25, 0.18, 0.14, 0.12], dtype=np.float32
)
# Macro-balanced action-kind weights so rare high-leverage decisions (`upgrade_tavern`, `choose_trinket`,
# `choose_hero`, `dark_discovery`) get clean gradient signal instead of being drowned out by `play`/`reposition`!
AWBC_KIND_BOOST: Dict[str, float] = {
    "upgrade_tavern": 6.0,
    "choose_trinket": 8.0,
    "choose_hero": 8.0,
    "dark_discovery": 5.0,
    "choose_discover": 2.5,
    "refresh": 2.0,
    "hero_power": 2.0,
    "activate": 2.0,
    "play_on_shop": 2.5,
}
REPOSITION_MASK_BOOL = ACTION_KIND_IDS == ACTION_KINDS.index("reposition")


def _act_to_idx(act: seaglass.TavernAction) -> int:
    a0, a1 = act.args
    return ACTION_TO_INDEX.get((act.kind, a0, a1), END_TURN_INDEX)


def _collect_one_game(args_t: Tuple[int, str, int]) -> Dict[str, np.ndarray]:
    seed, teacher_mix, beam_w = args_t
    from seaglass_bots.architect.hybrid import ArchitectBot
    from seaglass_bots.architect.zero_bot import ZeroPhysicsBot
    from seaglass_bots.sovereign_search import SovereignBot

    lobby = seaglass.LobbyGame(players=8, duos=False, seed=seed)
    bots: List[Any] = []
    for p in range(8):
        s_p = seed * 47 + p * 17 + 5
        if teacher_mix == "architect":
            bots.append(ArchitectBot(beam_width=beam_w, seed=s_p))
        elif teacher_mix == "mixed":
            if p < 4:
                bots.append(ArchitectBot(beam_width=beam_w, seed=s_p))
            elif p < 6:
                bots.append(
                    ZeroPhysicsBot(
                        beam_width=max(10, beam_w),
                        max_depth=5,
                        top_k_physics=4,
                        refresh_samples=3,
                        combats_eval=6,
                        neural_alpha=0.50,
                        two_turn_lookahead=False,
                        seed=s_p,
                    )
                )
            else:
                bots.append(SovereignBot(beam_width=beam_w, seed=s_p))
        else:
            bots.append(SovereignBot(beam_width=beam_w, seed=s_p))

    seats = [Seat(lobby, p) for p in range(8)]
    rec_p: List[List[Tuple]] = [[] for _ in range(8)]

    while not lobby.done:
        lobby.begin_phase()
        is_turn = lobby.phase == "turn"
        ctxs = {p: (lobby.context(p) if is_turn else None) for p in lobby.active_players()}
        for p in list(lobby.active_players()):
            steps = 0
            while not lobby.is_done(p) and steps < 60:
                obs = lobby.observe(p)
                enc = encode_observation(obs, ctxs.get(p), arranging=False)
                mk = action_mask(seats[p], allow_reposition=True, allow_freeze=True, obs=obs)
                act = bots[p].act_in_lobby(lobby, p)
                if not lobby.is_legal(p, act):
                    lobby.force_end(p)
                    break
                a_idx = _act_to_idx(act)
                mk[a_idx] = True
                ended = lobby.step(p, act)
                rec_p[p].append((enc[0], enc[1], enc[2].astype(np.float16), mk, a_idx, bool(ended or act.kind == "end_turn")))
                steps += 1
            if not lobby.is_done(p):
                lobby.force_end(p)
                if rec_p[p]:
                    last = rec_p[p][-1]
                    rec_p[p][-1] = (last[0], last[1], last[2], last[3], last[4], True)
        lobby.end_phase()

    places = list(lobby.placements)
    pv_l, ci_l, nf_l, mk_l, ac_l, ret_l, pl_l = [], [], [], [], [], [], []
    gamma = 0.97
    for p in range(8):
        pl = places[p]
        if pl is None or not rec_p[p]:
            continue
        pl_idx = max(0, min(7, int(pl) - 1))
        rew_final = float(FIRST_PLACE_REWARDS[pl_idx])
        T = len(rec_p[p])
        rews = np.zeros(T, dtype=np.float32)
        rews[-1] = rew_final
        t_ends = np.array([r[5] for r in rec_p[p]], dtype=np.bool_)
        _, rets = gae(rews, np.zeros(T, dtype=np.float32), t_ends, True, gamma, 1.0)
        for i, (pv, ci, nf, mk, ac, _) in enumerate(rec_p[p]):
            pv_l.append(pv)
            ci_l.append(ci)
            nf_l.append(nf)
            mk_l.append(mk)
            ac_l.append(ac)
            ret_l.append(rets[i])
            pl_l.append(pl_idx)

    return {
        "player_vec": np.stack(pv_l, axis=0).astype(np.float32),
        "slot_card_ids": np.stack(ci_l, axis=0).astype(np.int32),
        "slot_num_feats": np.stack(nf_l, axis=0).astype(np.float16),
        "action_mask": np.stack(mk_l, axis=0).astype(np.bool_),
        "actions": np.asarray(ac_l, dtype=np.int64),
        "returns": np.asarray(ret_l, dtype=np.float32),
        "place": np.asarray(pl_l, dtype=np.int8),
    }


def collect_expert_dataset(
    out_npz: str,
    games: int = 80,
    workers: int = 10,
    base_seed: int = 500000,
    teacher_mix: str = "mixed",
    beam_width: int = 12,
) -> None:
    t0 = time.perf_counter()
    ctx_mp = mp.get_context("spawn")
    jobs = [(base_seed + i * 137, teacher_mix, beam_width) for i in range(games)]
    chunks: List[Dict[str, np.ndarray]] = []
    with ctx_mp.Pool(processes=workers, maxtasksperchild=8) as pool:
        for idx, ch in enumerate(pool.imap_unordered(_collect_one_game, jobs, chunksize=1), 1):
            chunks.append(ch)
            if idx % 10 == 0 or idx == games:
                n_dec = sum(len(c["actions"]) for c in chunks)
                print(
                    f"[PolicyCollect {idx:3d}/{games}] {time.perf_counter()-t0:5.0f}s | decisions={n_dec}",
                    flush=True,
                )
    Path(out_npz).parent.mkdir(parents=True, exist_ok=True)
    np.savez_compressed(
        out_npz,
        **{k: np.concatenate([c[k] for c in chunks], axis=0) for k in chunks[0].keys()},
    )
    print(f"Saved expert policy dataset ({sum(len(c['actions']) for c in chunks)} steps) -> {out_npz}", flush=True)


def train_awbc_cuda(
    npz_paths: Sequence[str],
    out_ckpt: str,
    init_ckpt: Optional[str] = None,
    d_model: int = 128,
    layers: int = 4,
    heads: int = 4,
    ff: int = 256,
    epochs: int = 14,
    batch_size: int = 1024,
    lr: float = 8e-4,
    no_reposition: bool = False,
    macro_balance: bool = False,
    seed: int = 2029,
    device: str | None = None,
) -> None:
    torch.manual_seed(seed)
    rng = np.random.default_rng(seed)
    dev = pick_device(device)

    cols: Dict[str, List[np.ndarray]] = {
        k: [] for k in ("player_vec", "slot_card_ids", "slot_num_feats", "action_mask", "actions", "returns", "place")
    }
    for p in npz_paths:
        if not os.path.exists(p):
            continue
        d = np.load(p)
        for k in cols:
            cols[k].append(d[k])
    data = {k: np.concatenate(v, axis=0) for k, v in cols.items()}

    if no_reposition:
        is_repo = REPOSITION_MASK_BOOL[data["actions"]]
        keep = ~is_repo
        for k in list(data.keys()):
            data[k] = data[k][keep]
        data["action_mask"][:, REPOSITION_MASK_BOOL] = False
        print(f"Filtered out {int(is_repo.sum())} reposition steps -> {len(data['actions'])} pure strategy decisions", flush=True)

    # Precompute per-action-index macro-balance weight vector
    act_w_np = np.ones(ACTION_SPACE_SIZE, dtype=np.float32)
    if macro_balance:
        for ai, act_obj in enumerate(ACTIONS):
            act_w_np[ai] = AWBC_KIND_BOOST.get(act_obj.kind, 1.0)
    act_w_table = torch.from_numpy(act_w_np).to(dev)
    upg_idx = ACTION_TO_INDEX[("upgrade_tavern", None, None)]

    n = len(data["actions"])
    perm = rng.permutation(n)
    n_val = max(2048, int(0.08 * n))
    va_idx, tr_idx = perm[:n_val], perm[n_val:]

    if init_ckpt and os.path.exists(init_ckpt):
        model, _ = rl_ckpt.load(init_ckpt)
    else:
        model = EntityTransformer(
            card_embed_dim=24,
            d_model=d_model,
            layers=layers,
            heads=heads,
            ff=ff,
            head_dim=d_model,
            seed=seed,
            kind_prior=True,
            static_features=True,
        )
    model = model.to(dev)
    opt = torch.optim.AdamW(model.parameters(), lr=lr, weight_decay=1.5e-3)
    sched = torch.optim.lr_scheduler.CosineAnnealingLR(opt, T_max=max(1, epochs), eta_min=lr * 0.08)
    w_table = torch.from_numpy(AWBC_PLACE_WEIGHTS).to(dev)

    print(
        f"Training EntityTransformer AWBC (d={d_model}, L={layers}, no_repo={no_reposition}, macro_bal={macro_balance}) "
        f"on {dev} | train={len(tr_idx)} val={len(va_idx)}",
        flush=True,
    )

    best_val_ce = 1e9
    best_sd = None
    for ep in range(1, epochs + 1):
        model.train()
        rng.shuffle(tr_idx)
        tr_loss = 0.0
        steps = 0
        for s0 in range(0, len(tr_idx), batch_size):
            idx = tr_idx[s0 : s0 + batch_size]
            pv = torch.from_numpy(data["player_vec"][idx].astype(np.float32)).to(dev)
            ci = torch.from_numpy(data["slot_card_ids"][idx].astype(np.int64)).to(dev)
            nf = torch.from_numpy(data["slot_num_feats"][idx].astype(np.float32)).to(dev)
            mk = torch.from_numpy(data["action_mask"][idx]).to(dev)
            ac = torch.from_numpy(data["actions"][idx]).to(dev)
            rt = torch.from_numpy(data["returns"][idx].astype(np.float32)).to(dev)
            pl = torch.from_numpy(data["place"][idx].astype(np.int64)).to(dev)

            logits, val = model(pv, ci, nf, mk)
            ce_per = F.cross_entropy(logits, ac, reduction="none")
            w = w_table[pl] * act_w_table[ac]
            loss_pi = (ce_per * w).sum() / w.sum().clamp(min=1.0)
            loss_v = F.mse_loss(val, rt)
            loss = loss_pi + 0.5 * loss_v

            opt.zero_grad(set_to_none=True)
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            opt.step()
            tr_loss += float(loss.item())
            steps += 1
        sched.step()

        model.eval()
        v_ce, v_acc, v_acc1, v_n, v_n1, v_upg_ok, v_upg_n = 0.0, 0, 0, 0, 0, 0, 0
        with torch.no_grad():
            for s0 in range(0, len(va_idx), batch_size):
                idx = va_idx[s0 : s0 + batch_size]
                pv = torch.from_numpy(data["player_vec"][idx].astype(np.float32)).to(dev)
                ci = torch.from_numpy(data["slot_card_ids"][idx].astype(np.int64)).to(dev)
                nf = torch.from_numpy(data["slot_num_feats"][idx].astype(np.float32)).to(dev)
                mk = torch.from_numpy(data["action_mask"][idx]).to(dev)
                ac = torch.from_numpy(data["actions"][idx]).to(dev)
                pl = torch.from_numpy(data["place"][idx].astype(np.int64)).to(dev)
                logits, _ = model(pv, ci, nf, mk)
                v_ce += float(F.cross_entropy(logits, ac, reduction="sum").item())
                pred = logits.argmax(dim=-1)
                v_acc += int((pred == ac).sum().item())
                v_n += len(idx)
                m1 = pl == 0
                if bool(m1.any()):
                    v_acc1 += int((pred[m1] == ac[m1]).sum().item())
                    v_n1 += int(m1.sum().item())
                mu = ac == upg_idx
                if bool(mu.any()):
                    v_upg_ok += int((pred[mu] == ac[mu]).sum().item())
                    v_upg_n += int(mu.sum().item())
        val_ce = v_ce / max(1, v_n)
        if val_ce < best_val_ce:
            best_val_ce = val_ce
            best_sd = {k: v.detach().cpu().clone() for k, v in model.state_dict().items()}
        print(
            f"  AWBC Ep {ep:2d}/{epochs} | tr={tr_loss/max(1,steps):.4f} "
            f"| val_ce={val_ce:.4f} | acc={v_acc/max(1,v_n):.1%} | 1st_acc={v_acc1/max(1,v_n1):.1%} "
            f"| upg_acc={v_upg_ok/max(1,v_upg_n):.1%}",
            flush=True,
        )

    if best_sd is not None:
        model.load_state_dict(best_sd)
    model_cpu = model.cpu().eval()
    Path(out_ckpt).parent.mkdir(parents=True, exist_ok=True)
    rl_ckpt.save(out_ckpt, model_cpu, iteration=0, extra={"awbc_val_ce": best_val_ce})
    print(f"Saved EntityTransformer AWBC -> {out_ckpt} (val_ce={best_val_ce:.4f})", flush=True)


def ppo_update_cuda(
    model: nn.Module,
    optimizer: torch.optim.Optimizer,
    batch: Dict[str, torch.Tensor],
    dev: torch.device,
    epochs: int = 4,
    minibatch_size: int = 2048,
    clip_eps: float = 0.18,
    vf_coef: float = 0.5,
    ent_coef: float = 0.004,
    target_kl: float = 0.025,
    ref_model: Optional[nn.Module] = None,
    ref_kl_coef: float = 0.0,
) -> Dict[str, float]:
    n = batch["actions"].shape[0]
    adv = batch["advantages"].float()
    adv = (adv - adv.mean()) / (adv.std() + 1e-8) if n > 1 else adv
    model.train()
    stats: Dict[str, List[float]] = {"pi": [], "v": [], "ent": [], "kl": [], "rkl": []}
    stop = False
    for _ in range(epochs):
        perm = torch.randperm(n)
        for s0 in range(0, n, minibatch_size):
            idx = perm[s0 : s0 + minibatch_size]
            pv = batch["player_vec"][idx].float().to(dev)
            ci = batch["slot_card_ids"][idx].to(dev)
            nf = batch["slot_num_feats"][idx].to(dev)
            mk = batch["action_mask"][idx].to(dev)
            ac = batch["actions"][idx].to(dev)
            olp = batch["old_log_probs"][idx].float().to(dev)
            ret = batch["returns"][idx].float().to(dev)
            a = adv[idx].to(dev)

            logits, values = model(pv, ci, nf, mk)
            lps = torch.log_softmax(logits, dim=-1)
            new_lp = lps.gather(1, ac.unsqueeze(-1)).squeeze(-1)
            probs = lps.exp()
            ent = -(probs * lps.clamp(min=-20.0)).sum(-1).mean()
            lr_t = new_lp - olp
            ratio = lr_t.exp()
            l_pi = -torch.min(ratio * a, ratio.clamp(1.0 - clip_eps, 1.0 + clip_eps) * a).mean()
            l_v = 0.5 * (values - ret).pow(2).mean()
            loss = l_pi + vf_coef * l_v - ent_coef * ent

            rkl_val = 0.0
            if ref_model is not None and ref_kl_coef > 0.0:
                with torch.no_grad():
                    ref_logits, _ = ref_model(pv, ci, nf, mk)
                    ref_lps = torch.log_softmax(ref_logits, dim=-1)
                # Forward KL(pi || pi_ref) over legal actions (AlphaStar teacher anchor!)
                rkl = (probs * (lps.clamp(min=-20.0) - ref_lps.clamp(min=-20.0))).sum(dim=-1).mean()
                loss = loss + ref_kl_coef * rkl
                rkl_val = float(rkl.item())

            optimizer.zero_grad(set_to_none=True)
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), 0.5)
            optimizer.step()
            with torch.no_grad():
                kl = float(((ratio - 1.0) - lr_t).mean().item())
            stats["pi"].append(float(l_pi.item()))
            stats["v"].append(float(l_v.item()))
            stats["ent"].append(float(ent.item()))
            stats["kl"].append(kl)
            stats["rkl"].append(rkl_val)
            if kl > 1.5 * target_kl:
                stop = True
                break
        if stop:
            break
    model.eval()
    return {k: float(np.mean(v)) if v else 0.0 for k, v in stats.items()}


def run_ppo_cuda(
    init_ckpt: str,
    run_dir: str,
    iterations: int = 200,
    games_per_iter: int = 40,
    workers: int = 6,
    lr: float = 1.1e-4,
    p_tempo_bot: float = 0.05,
    p_boss: float = 0.25,
    boss_kind: str = "apex",
    ref_ckpt: str = "",
    ref_kl_coef: float = 0.025,
    shaping: float = 0.0,
    gamma: float = 0.995,
    no_reposition: bool = False,
    anchor_ckpts: Sequence[str] = (),
    seed: int = 2029,
    device: str | None = None,
) -> None:
    import gc

    dev = pick_device(device)
    run = Path(run_dir)
    (run / "snapshots").mkdir(parents=True, exist_ok=True)
    model, _ = rl_ckpt.load(init_ckpt)
    model = model.to(dev)
    model.eval()

    ref_model = None
    if ref_kl_coef > 0.0:
        rp = ref_ckpt if (ref_ckpt and os.path.exists(ref_ckpt)) else init_ckpt
        ref_model, _ = rl_ckpt.load(rp)
        ref_model = ref_model.to(dev)
        ref_model.eval()
        for p_param in ref_model.parameters():
            p_param.requires_grad_(False)

    opt = torch.optim.AdamW(model.parameters(), lr=lr, eps=1e-5, weight_decay=1e-4)
    rng = random.Random(seed)
    config = LobbyConfig(duos=False, players=8, max_turns=30, heroes=True, damage_caps=True, tribes=5)
    opts = PlayOptions(
        max_steps_per_turn=42,
        positioning_phase=False,
        allow_reposition=not no_reposition,
        allow_freeze=True,
        prune_dominated=False,
        deterministic=False,
        gamma=gamma,
        shaping=shaping,
        health_shaping=0.0,
    )
    anchors = [p for p in anchor_ckpts if p and os.path.exists(p)]
    if init_ckpt not in anchors:
        anchors.insert(0, init_ckpt)
    dyn_pool: List[str] = []
    player = Player(workers)
    t0_all = time.perf_counter()
    print(
        f"Starting AlphaStar-League CUDA PPO ({iterations} iters x {games_per_iter} games, "
        f"gamma={gamma}, shaping={shaping}, boss={boss_kind}@{p_boss}, ref_kl={ref_kl_coef}, anchors={len(anchors)}) on {dev}",
        flush=True,
    )

    def _save_cpu_ckpt(path: str, it_num: int) -> None:
        cpu_m = EntityTransformer(**model.config)
        cpu_m.load_state_dict({k: v.detach().cpu() for k, v in model.state_dict().items()})
        cpu_m.eval()
        rl_ckpt.save(path, cpu_m, iteration=it_num)

    try:
        for it in range(1, iterations + 1):
            t0 = time.perf_counter()
            # Recycle worker pool every 30 iterations to keep RAM strictly flat!
            if it > 1 and (it - 1) % 30 == 0:
                player.close()
                gc.collect()
                player = Player(workers)

            frac = 0.25 + 0.75 * 0.5 * (1.0 + np.cos(np.pi * (it - 1) / max(1, iterations)))
            for pg in opt.param_groups:
                pg["lr"] = lr * float(frac)

            pool = anchors[-2:] + dyn_pool[-2:]
            specs: List[GameSpec] = []
            for g in range(games_per_iter):
                mine = set(rng.sample(range(8), 4))
                ctrls = []
                for u in range(8):
                    if u in mine:
                        ctrls.append(LEARNER)
                    else:
                        r_u = rng.random()
                        if p_boss > 0.0 and r_u < p_boss:
                            ctrls.append(boss_kind)
                        elif r_u < p_boss + p_tempo_bot:
                            ctrls.append("9kgamerbot-tempo")
                        else:
                            ctrls.append(rng.choice(pool))
                specs.append(GameSpec(seed=seed * 10000 + it * 1000 + g, controllers=ctrls))
            src = (dict(model.config), {k: v.detach().cpu().clone() for k, v in model.state_dict().items()})
            sources: Dict[str, Any] = {LEARNER: src}
            sources.update({p: p for p in pool})
            results = player.play(config, specs, sources, opts, seed=it)
            t_play = time.perf_counter() - t0

            trajs = []
            places = []
            for r in results:
                for u in range(8):
                    if r.controllers[u] == LEARNER:
                        places.append(r.placements[u])
                l_seats = [u for u in range(8) if r.controllers[u] == LEARNER]
                for tr, u in zip(r.trajectories, l_seats):
                    pl = int(r.placements[u])
                    lin_r = 1.0 - 2.0 * (pl - 1) / 7.0
                    sharp_r = float(FIRST_PLACE_REWARDS[max(0, min(7, pl - 1))])
                    if len(tr["rewards"]) > 0:
                        tr["rewards"][-1] += sharp_r - lin_r
                    trajs.append(tr)

            batch = build_batch(trajs, opts.gamma, 0.95)
            n_steps = int(len(batch["actions"]))
            m = ppo_update_cuda(
                model,
                opt,
                batch,
                dev=dev,
                epochs=4,
                minibatch_size=2048,
                ent_coef=0.005,
                ref_model=ref_model,
                ref_kl_coef=ref_kl_coef,
            )
            first_rate = sum(1 for pl in places if pl == 1) / max(1, len(places))
            avg_pl = float(np.mean(places))
            if it % 10 == 0:
                sp = str(run / "snapshots" / f"iter_{it:04d}.pt")
                _save_cpu_ckpt(sp, it)
                dyn_pool = (dyn_pool + [sp])[-2:]
            if it % 10 == 0 or it == iterations:
                _save_cpu_ckpt(str(run / "latest.pt"), it)
            print(
                f"PPO it {it:3d}/{iterations} | avg={avg_pl:.2f} 1st={first_rate:.1%} "
                f"| steps={n_steps:6d} | kl={m['kl']:.4f} rkl={m['rkl']:.3f} ent={m['ent']:.2f} "
                f"| play={t_play:.1f}s upd={time.perf_counter()-t0-t_play:.1f}s ({time.perf_counter()-t0_all:.0f}s)",
                flush=True,
            )
            del results, trajs, batch
            if it % 4 == 0:
                gc.collect()
                empty_cache(dev)
    finally:
        player.close()


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--mode", choices=["collect", "awbc", "ppo"], required=True)
    p.add_argument("--dataset", type=str, default="/tmp/policy_expert.npz")
    p.add_argument("--extra-datasets", type=str, nargs="*", default=[])
    p.add_argument("--teacher-mix", choices=["mixed", "architect", "sovereign"], default="mixed")
    p.add_argument("--games", type=int, default=80)
    p.add_argument(
        "--workers",
        type=int,
        default=default_workers(cap=10),
        help="self-play/collection worker processes (default: $SEAGLASS_WORKERS, else half the cores, at most 10)",
    )
    p.add_argument("--base-seed", type=int, default=500000)
    p.add_argument("--beam-width", type=int, default=12)
    p.add_argument("--out-ckpt", type=str, default="/tmp/policy_awbc.pt")
    p.add_argument("--init-ckpt", type=str, default="")
    p.add_argument("--run-dir", type=str, default="/tmp/ppo_run")
    p.add_argument("--epochs", type=int, default=14)
    p.add_argument("--iterations", type=int, default=80)
    p.add_argument("--lr", type=float, default=8e-4)
    p.add_argument("--d-model", type=int, default=128)
    p.add_argument("--layers", type=int, default=4)
    p.add_argument("--shaping", type=float, default=0.0)
    p.add_argument("--gamma", type=float, default=0.995)
    p.add_argument("--p-tempo", type=float, default=0.05)
    p.add_argument("--p-boss", type=float, default=0.25)
    p.add_argument("--boss-kind", type=str, default="apex")
    p.add_argument("--ref-ckpt", type=str, default="")
    p.add_argument("--ref-kl-coef", type=float, default=0.025)
    p.add_argument("--no-reposition", action="store_true")
    p.add_argument("--macro-balance", action="store_true")
    p.add_argument("--anchor-ckpts", type=str, nargs="*", default=[])
    add_device_arg(p)
    args = p.parse_args()
    torch.set_num_threads(default_threads())

    if args.mode == "collect":
        collect_expert_dataset(
            out_npz=args.dataset,
            games=args.games,
            workers=args.workers,
            base_seed=args.base_seed,
            teacher_mix=args.teacher_mix,
            beam_width=args.beam_width,
        )
    elif args.mode == "awbc":
        train_awbc_cuda(
            npz_paths=[args.dataset] + list(args.extra_datasets),
            out_ckpt=args.out_ckpt,
            init_ckpt=args.init_ckpt or None,
            d_model=args.d_model,
            layers=args.layers,
            ff=args.d_model * 2,
            epochs=args.epochs,
            lr=args.lr,
            no_reposition=args.no_reposition,
            macro_balance=args.macro_balance,
            device=args.device,
        )
    elif args.mode == "ppo":
        run_ppo_cuda(
            init_ckpt=args.init_ckpt or args.out_ckpt,
            run_dir=args.run_dir,
            iterations=args.iterations,
            games_per_iter=args.games,
            workers=args.workers,
            lr=args.lr,
            p_tempo_bot=args.p_tempo,
            p_boss=args.p_boss,
            boss_kind=args.boss_kind,
            ref_ckpt=args.ref_ckpt,
            ref_kl_coef=args.ref_kl_coef,
            shaping=args.shaping,
            gamma=args.gamma,
            no_reposition=args.no_reposition,
            anchor_ckpts=args.anchor_ckpts,
            device=args.device,
        )


if __name__ == "__main__":
    main()
