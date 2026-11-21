"""High-Throughput Distributional-Critic League CUDA PPO (`train_dist_ppo.py`).

100% Pure End-to-End RL (ZERO hardcoded card/comp/hero/trinket/leveling rules!):
  1. Trains `DistEntityTransformer` with an **8-Way Categorical Placement Critic**
     (Ordinal-smoothed CE + 1st-Place Binary BCE -> dramatically lower critic variance in 8p FFA!).
  2. Applies `<0.1ms` pure-keyword EOT positioning (`--fast-pos`, zero card names) on every
     turn end during training so combat outcomes match evaluation physics!
  3. Runs fast batched lockstep rollouts (~15x faster than all-Sovereign lobbies) mixing:
       - 4x LEARNER seats
       - 4x League opponents (frozen historical snapshots + strong Tier-5/6 anchors + optional
         live `SovereignBot` boss seat in `p_sov_lobby` fraction of lobbies so Tier-6 ceiling
         pressure never disappears!).
  4. Recycles worker pool periodically to keep RAM strictly flat on 12 GiB hosts.
"""

from __future__ import annotations

import argparse
import gc
import multiprocessing as mp
import os
import random
import sys
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

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
import torch.nn.functional as F

from seaglass_rl import checkpoint as rl_ckpt
from seaglass_rl.actions import END_TURN_INDEX, action_from_index, action_mask
from seaglass_rl.device import add_device_arg, default_threads, default_workers, empty_cache, pick_device
from seaglass_rl.features import encode_observation
from seaglass_rl.model import sample_categorical
from seaglass_rl.play import Seat
from seaglass_rl.ppo import build_batch

from .dist_transformer import DIST_PLACE_ATOMS, DistEntityTransformer
from .train_anti_sov import _fast_keyword_reposition_lobby

_MODEL_CACHE: Dict[str, Tuple[float, Any]] = {}


def _load_cached(path: str) -> Any:
    mt = os.path.getmtime(path)
    hit = _MODEL_CACHE.get(path)
    if hit is not None and hit[0] == mt:
        return hit[1]
    ck = torch.load(path, map_location="cpu", weights_only=False)
    if ck.get("model_class") == "DistEntityTransformer":
        m = DistEntityTransformer.load_checkpoint(path)
    else:
        m, _ = rl_ckpt.load(path)
        m.eval()
    # Cap cache size cleanly to keep worker RAM tiny
    if len(_MODEL_CACHE) > 5:
        _MODEL_CACHE.clear()
    _MODEL_CACHE[path] = (mt, m)
    return m


def _play_league_game(
    args_t: Tuple[int, str, List[str], bool, bool]
) -> Dict[str, Any]:
    """Plays one 8-player FFA game with 4 LEARNER seats + 4 opponent controllers."""
    seed, learner_ckpt, opp_specs, no_repo, fast_pos = args_t
    torch.set_num_threads(1)
    rng_np = np.random.default_rng(seed)
    rng_py = random.Random(seed)

    learner_seats = set(rng_py.sample(range(8), 4))
    opp_seats = [p for p in range(8) if p not in learner_seats]
    ctrl_by_seat: Dict[int, str] = {}
    for p in learner_seats:
        ctrl_by_seat[p] = "__LEARNER__"
    for p, spec in zip(opp_seats, opp_specs):
        ctrl_by_seat[p] = spec

    # Load unique neural models & any live SovereignBot bosses
    models_by_key: Dict[str, Any] = {"__LEARNER__": _load_cached(learner_ckpt)}
    sov_bots: Dict[int, Any] = {}
    for p, spec in ctrl_by_seat.items():
        if spec == "__LEARNER__":
            continue
        if spec == "__SOV__":
            from seaglass_bots.sovereign_search import SovereignBot

            sb = SovereignBot(
                beam_width=4,
                max_epoch_depth=4,
                top_k_combat_evals=2,
                refresh_samples=1,
                num_particles=1,
                seed=seed * 31 + p * 17 + 3,
            )
            sb.evaluator.positioner.max_candidates = 12
            sb.positioner.max_candidates = 12
            sov_bots[p] = sb
        elif spec not in models_by_key:
            models_by_key[spec] = _load_cached(spec)

    lobby = seaglass.LobbyGame(players=8, duos=False, seed=seed)
    seats = [Seat(lobby, p) for p in range(8)]

    tr_buf: Dict[int, Dict[str, List]] = {
        p: {
            "player_vec": [],
            "slot_card_ids": [],
            "slot_num_feats": [],
            "action_mask": [],
            "actions": [],
            "log_probs": [],
            "values": [],
            "rewards": [],
            "turn_end": [],
        }
        for p in learner_seats
    }

    while not lobby.done:
        lobby.begin_phase()
        is_turn = lobby.phase == "turn"
        ctxs = {p: (lobby.context(p) if is_turn else None) for p in lobby.active_players()}

        # 1. Step any live SovereignBot boss seats
        for p in list(lobby.active_players()):
            if p not in sov_bots:
                continue
            steps = 0
            while not lobby.is_done(p) and steps < 55:
                act = sov_bots[p].act_in_lobby(lobby, p)
                if not lobby.is_legal(p, act):
                    lobby.force_end(p)
                    break
                lobby.step(p, act)
                steps += 1
            if not lobby.is_done(p):
                lobby.force_end(p)

        # 2. Step neural seats grouped by model key in batched lockstep!
        step_cnt = {p: 0 for p in range(8) if p not in sov_bots}
        while True:
            todo = [p for p in lobby.active_players() if p in step_cnt and step_cnt[p] < 44]
            if not todo:
                for p in list(lobby.active_players()):
                    if p in step_cnt:
                        if fast_pos and is_turn:
                            _fast_keyword_reposition_lobby(lobby, p)
                        lobby.force_end(p)
                        if p in tr_buf and tr_buf[p]["turn_end"]:
                            tr_buf[p]["turn_end"][-1] = True
                break

            groups: Dict[str, List[int]] = {}
            for p in todo:
                groups.setdefault(ctrl_by_seat[p], []).append(p)

            for mkey, plist in groups.items():
                mod = models_by_key[mkey]
                pvs, cis, nfs, mks, valid_p = [], [], [], [], []
                for p in plist:
                    obs = lobby.observe(p)
                    mk = action_mask(seats[p], allow_reposition=not no_repo, allow_freeze=True, obs=obs)
                    if not mk.any():
                        if fast_pos and is_turn:
                            _fast_keyword_reposition_lobby(lobby, p)
                        lobby.force_end(p)
                        if p in tr_buf and tr_buf[p]["turn_end"]:
                            tr_buf[p]["turn_end"][-1] = True
                        continue
                    pv, ci, nf = encode_observation(obs, ctxs.get(p), arranging=False)
                    pvs.append(pv)
                    cis.append(ci)
                    nfs.append(nf)
                    mks.append(mk)
                    valid_p.append(p)
                if not valid_p:
                    continue
                with torch.no_grad():
                    logits, vals = mod.forward(
                        torch.from_numpy(np.stack(pvs, axis=0).astype(np.float32)),
                        torch.from_numpy(np.stack(cis, axis=0).astype(np.int64)),
                        torch.from_numpy(np.stack(nfs, axis=0).astype(np.float32)),
                        torch.from_numpy(np.stack(mks, axis=0).astype(np.bool_)),
                    )
                    lps = torch.log_softmax(logits, dim=-1).numpy()
                    v_np = vals.numpy()
                acts = sample_categorical(np.exp(lps), rng_np)
                for i, p in enumerate(valid_p):
                    a_i = int(acts[i])
                    act = action_from_index(a_i)
                    if not lobby.is_legal(p, act):
                        a_i = END_TURN_INDEX
                        act = action_from_index(a_i)
                    if fast_pos and is_turn and a_i == END_TURN_INDEX:
                        _fast_keyword_reposition_lobby(lobby, p)
                    ended = lobby.step(p, act)
                    step_cnt[p] += 1
                    if p in tr_buf:
                        b = tr_buf[p]
                        b["player_vec"].append(pvs[i])
                        b["slot_card_ids"].append(cis[i])
                        b["slot_num_feats"].append(nfs[i].astype(np.float16))
                        b["action_mask"].append(mks[i])
                        b["actions"].append(a_i)
                        b["log_probs"].append(float(lps[i, a_i]))
                        b["values"].append(float(v_np[i]))
                        b["rewards"].append(0.0)
                        b["turn_end"].append(bool(ended or a_i == END_TURN_INDEX))

        lobby.end_phase()

    places = [int(x) for x in lobby.placements]
    trajs = []
    l_places, l_tiers, l_stats = [], [], []
    s_places = []
    for p in range(8):
        o = lobby.observe(p)
        if p in sov_bots:
            s_places.append(places[p])
        if p in learner_seats:
            l_places.append(places[p])
            l_tiers.append(int(o.tavern_tier))
            l_stats.append(int(sum(u.attack + u.health for u in o.board)))
            b = tr_buf[p]
            if b["actions"]:
                pl_idx = max(0, min(7, places[p] - 1))
                b["rewards"][-1] = float(DIST_PLACE_ATOMS[pl_idx])
                b["turn_end"][-1] = True
                trajs.append(
                    {
                        "player_vec": np.asarray(b["player_vec"], dtype=np.float32),
                        "slot_card_ids": np.asarray(b["slot_card_ids"], dtype=np.int32),
                        "slot_num_feats": np.asarray(b["slot_num_feats"], dtype=np.float16),
                        "action_mask": np.asarray(b["action_mask"], dtype=np.bool_),
                        "actions": np.asarray(b["actions"], dtype=np.int64),
                        "log_probs": np.asarray(b["log_probs"], dtype=np.float32),
                        "values": np.asarray(b["values"], dtype=np.float32),
                        "rewards": np.asarray(b["rewards"], dtype=np.float32),
                        "turn_end": np.asarray(b["turn_end"], dtype=np.bool_),
                        "place_idx": np.full(len(b["actions"]), pl_idx, dtype=np.int64),
                        "done": np.asarray(True),
                    }
                )

    return {
        "trajs": trajs,
        "l_places": l_places,
        "l_tiers": l_tiers,
        "l_stats": l_stats,
        "s_places": s_places,
    }


def _ordinal_targets(place_idx: torch.Tensor, sigma: float = 0.55) -> torch.Tensor:
    ranks = torch.arange(8, device=place_idx.device, dtype=torch.float32)[None, :]
    d2 = (ranks - place_idx.float()[:, None]) ** 2
    w = torch.exp(-0.5 * d2 / (sigma * sigma))
    return w / w.sum(dim=-1, keepdim=True)


def dist_ppo_update_cuda(
    model: DistEntityTransformer,
    opt: torch.optim.Optimizer,
    batch: Dict[str, np.ndarray],
    place_idx_all: np.ndarray,
    dev: torch.device,
    epochs: int = 4,
    minibatch_size: int = 2048,
    clip_eps: float = 0.2,
    vf_coef: float = 0.65,
    ent_coef: float = 0.006,
    ref_model: Optional[torch.nn.Module] = None,
    ref_kl_coef: float = 0.0,
    critic_only: bool = False,
) -> Dict[str, float]:
    n = len(batch["actions"])
    adv = batch["advantages"].float()
    adv = (adv - adv.mean()) / (adv.std() + 1e-8)

    pv_all = batch["player_vec"].float()
    ci_all = batch["slot_card_ids"].long()
    nf_all = batch["slot_num_feats"].float()
    mk_all = batch["action_mask"].bool()
    ac_all = batch["actions"].long()
    lp_all = batch["old_log_probs"].float()
    ad_all = adv
    pl_all = torch.from_numpy(place_idx_all.astype(np.int64))

    # Measure TRUE pre-update unseen-game `val_ce` on this iteration's freshly played trajectories!
    model.eval()
    v_ce_pre = 0.0
    with torch.inference_mode():
        for s0 in range(0, n, 2048):
            e0 = min(n, s0 + 2048)
            _, _, pl_lg = model.forward_with_dist(
                pv_all[s0:e0].to(dev), ci_all[s0:e0].to(dev), nf_all[s0:e0].to(dev), None
            )
            v_ce_pre += float(F.cross_entropy(pl_lg, pl_all[s0:e0].to(dev), reduction="sum").item())
    val_ce = v_ce_pre / max(1, n)

    rng = np.random.default_rng()
    model.train()
    # During critic-only warmup, freeze ALL shared encoder & actor weights so random critic gradients
    # NEVER corrupt the warm-started policy (guarantees kl == 0.0000 during warmup!).
    if critic_only:
        for name, pm in model.named_parameters():
            pm.requires_grad_(name.startswith("dist_critic."))
    else:
        for pm in model.parameters():
            pm.requires_grad_(True)
    tot_kl = tot_ent = tot_ce = tot_rkl = 0.0
    steps = 0

    for _ in range(epochs):
        order = rng.permutation(n)
        for s0 in range(0, n, minibatch_size):
            idx = order[s0 : s0 + minibatch_size]
            if len(idx) < 32:
                continue
            pv = pv_all[idx].to(dev, non_blocking=True)
            ci = ci_all[idx].to(dev, non_blocking=True)
            nf = nf_all[idx].to(dev, non_blocking=True)
            mk = mk_all[idx].to(dev, non_blocking=True)
            ac = ac_all[idx].to(dev, non_blocking=True)
            old_lp = lp_all[idx].to(dev, non_blocking=True)
            ad = ad_all[idx].to(dev, non_blocking=True)
            pl = pl_all[idx].to(dev, non_blocking=True)

            logits, _val, pl_logits = model.forward_with_dist(pv, ci, nf, mk)
            lps = F.log_softmax(logits, dim=-1)
            new_lp = lps.gather(1, ac[:, None]).squeeze(1)
            log_ratio = (new_lp - old_lp).clamp(-10.0, 10.0)
            ratio = torch.exp(log_ratio)
            surr1 = ratio * ad
            surr2 = ratio.clamp(1.0 - clip_eps, 1.0 + clip_eps) * ad
            pg_loss = -torch.min(surr1, surr2).mean()

            # Distributional 8-Way Placement Cross-Entropy + explicit 1st-Place Binary BCE!
            soft_t = _ordinal_targets(pl, sigma=0.55)
            pl_logp = F.log_softmax(pl_logits, dim=-1)
            ce_loss = -(soft_t * pl_logp).sum(dim=-1).mean()
            p_first = F.softmax(pl_logits, dim=-1)[:, 0].clamp(1e-5, 1.0 - 1e-5)
            f1_loss = F.binary_cross_entropy(p_first, (pl == 0).float())
            vf_loss = ce_loss + 0.50 * f1_loss

            probs = torch.exp(lps) * mk.float()
            ent = -(probs * lps.masked_fill(~mk, 0.0)).sum(dim=-1).mean()

            if critic_only:
                loss = vf_loss
                rkl_v = 0.0
            else:
                loss = pg_loss + vf_coef * vf_loss - ent_coef * ent
                rkl_v = 0.0
                if ref_model is not None and ref_kl_coef > 0.0:
                    with torch.no_grad():
                        ref_logits, _ = ref_model.forward(pv, ci, nf, mk)
                        ref_lps = F.log_softmax(ref_logits, dim=-1)
                    kl_ref = (probs * (lps - ref_lps).masked_fill(~mk, 0.0)).sum(dim=-1).mean()
                    loss = loss + ref_kl_coef * kl_ref
                    rkl_v = float(kl_ref.item())

            opt.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), 0.7)
            opt.step()

            with torch.no_grad():
                approx_kl = float(((ratio - 1.0) - log_ratio).mean().item())
            tot_kl += approx_kl
            tot_ent += float(ent.item())
            tot_ce += float(ce_loss.item())
            tot_rkl += rkl_v
            steps += 1
        if not critic_only and steps > 0 and (tot_kl / steps) > 0.035:
            break

    model.eval()
    return {
        "kl": tot_kl / max(1, steps),
        "ent": tot_ent / max(1, steps),
        "val_ce": val_ce,
        "v_ce": tot_ce / max(1, steps),
        "rkl": tot_rkl / max(1, steps),
    }


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--init-ckpt", required=True)
    p.add_argument("--ref-ckpt", default="")
    p.add_argument("--ref-kl-coef", type=float, default=0.006)
    p.add_argument("--anchor-ckpts", nargs="*", default=[])
    p.add_argument("--run-dir", required=True)
    p.add_argument("--iterations", type=int, default=200)
    p.add_argument("--games", type=int, default=24)
    p.add_argument(
        "--workers",
        type=int,
        default=default_workers(cap=6),
        help="self-play worker processes (default: $SEAGLASS_WORKERS, else half the cores, at most 6)",
    )
    p.add_argument("--p-sov-lobby", type=float, default=0.30, help="Fraction of lobbies containing live fast SovereignBot boss(es)")
    p.add_argument("--n-sov-boss", type=int, default=1, help="Number of live fast SovereignBot boss seats per boss lobby")
    p.add_argument("--ent-coef", type=float, default=0.005)
    p.add_argument("--lr", type=float, default=1.0e-4)
    p.add_argument("--gamma", type=float, default=0.995)
    p.add_argument("--no-reposition", action="store_true")
    p.add_argument("--fast-pos", action="store_true")
    p.add_argument("--seed", type=int, default=909)
    add_device_arg(p)
    args = p.parse_args()
    torch.set_num_threads(default_threads())

    dev = pick_device(args.device)
    run = Path(args.run_dir)
    (run / "snapshots").mkdir(parents=True, exist_ok=True)

    ck_init = torch.load(args.init_ckpt, map_location="cpu", weights_only=False)
    is_resume_dist = ck_init.get("model_class") == "DistEntityTransformer"
    if is_resume_dist:
        model = DistEntityTransformer.load_checkpoint(args.init_ckpt).to(dev)
    else:
        model = DistEntityTransformer.from_entity_checkpoint(args.init_ckpt).to(dev)
    warmup_iters = 0 if is_resume_dist else 3
    model.eval()

    ref_model = None
    if args.ref_kl_coef > 0.0:
        rp = args.ref_ckpt if (args.ref_ckpt and os.path.exists(args.ref_ckpt)) else args.init_ckpt
        ck_r = torch.load(rp, map_location="cpu", weights_only=False)
        if ck_r.get("model_class") == "DistEntityTransformer":
            ref_model = DistEntityTransformer.load_checkpoint(rp).to(dev)
        else:
            ref_model, _ = rl_ckpt.load(rp)
            ref_model = ref_model.to(dev)
        ref_model.eval()
        for pm in ref_model.parameters():
            pm.requires_grad_(False)

    opt = torch.optim.AdamW(model.parameters(), lr=args.lr, eps=1e-5, weight_decay=1e-4)
    rng = random.Random(args.seed)
    anchors = [a for a in args.anchor_ckpts if a and os.path.exists(a)]
    if args.init_ckpt not in anchors:
        anchors.insert(0, args.init_ckpt)
    dyn_pool: List[str] = []

    tmp_ckpt = str(run / "_worker_current.pt")
    ctx_mp = mp.get_context("spawn")
    pool = ctx_mp.Pool(processes=args.workers)
    t0_all = time.perf_counter()
    print(
        f"Starting Distributional-Critic League CUDA PPO ({args.iterations} iters x {args.games}g, "
        f"p_sov={args.p_sov_lobby}, fast_pos={args.fast_pos}, ref_kl={args.ref_kl_coef}) on {dev}",
        flush=True,
    )

    try:
        for it in range(1, args.iterations + 1):
            if it > 1 and (it - 1) % 25 == 0:
                pool.close()
                pool.join()
                gc.collect()
                pool = ctx_mp.Pool(processes=args.workers)

            t0 = time.perf_counter()
            model.save_checkpoint(tmp_ckpt, iteration=it - 1)

            # Cosine LR schedule (higher LR on `dist_critic` during frozen-encoder warmup)
            frac = 0.25 + 0.75 * 0.5 * (1.0 + np.cos(np.pi * (it - 1) / max(1, args.iterations)))
            for pg in opt.param_groups:
                pg["lr"] = 8.0e-4 if it <= warmup_iters else (args.lr * float(frac))

            opp_pool = anchors[-2:] + dyn_pool[-3:]
            jobs = []
            for g in range(args.games):
                opps = [rng.choice(opp_pool) for _ in range(4)]
                if args.p_sov_lobby > 0.0 and rng.random() < args.p_sov_lobby:
                    for b_i in range(min(args.n_sov_boss, 4)):
                        opps[b_i] = "__SOV__"
                jobs.append((args.seed * 10000 + it * 1000 + g, tmp_ckpt, opps, args.no_reposition, args.fast_pos))

            res_list = pool.map(_play_league_game, jobs)
            t_play = time.perf_counter() - t0

            trajs = [t for r in res_list for t in r["trajs"]]
            pl_all = np.concatenate([t["place_idx"] for t in trajs], axis=0)
            lp = [x for r in res_list for x in r["l_places"]]
            lt = [x for r in res_list for x in r["l_tiers"]]
            ls = [x for r in res_list for x in r["l_stats"]]
            sp = [x for r in res_list for x in r["s_places"]]

            batch = build_batch(trajs, args.gamma, 0.95)
            n_steps = int(len(batch["actions"]))
            m = dist_ppo_update_cuda(
                model,
                opt,
                batch,
                pl_all,
                dev=dev,
                epochs=4,
                minibatch_size=2048,
                vf_coef=0.35,
                ent_coef=args.ent_coef,
                ref_model=ref_model,
                ref_kl_coef=args.ref_kl_coef,
                critic_only=(it <= warmup_iters),
            )

            l_1st = sum(1 for x in lp if x == 1) / max(1, len(lp))
            if it % 10 == 0 or it == args.iterations:
                sp_path = str(run / "snapshots" / f"iter_{it:04d}.pt")
                model.save_checkpoint(sp_path, iteration=it)
                dyn_pool = (dyn_pool + [sp_path])[-3:]
            if it % 5 == 0 or it == args.iterations:
                model.save_checkpoint(str(run / "latest.pt"), iteration=it)

            sov_str = f"Sov={np.mean(sp):.2f}({len(sp)})" if sp else "Sov=none"
            tag = " [WARMUP]" if it <= warmup_iters else ""
            print(
                f"DistPPO it {it:3d}/{args.iterations}{tag} | L_avg={np.mean(lp):.2f} L_1st={l_1st:.1%} "
                f"L_T={np.mean(lt):.2f} medS={np.median(ls):.0f} | {sov_str} "
                f"| steps={n_steps:5d} val_ce={m['val_ce']:.3f} kl={m['kl']:.4f} ent={m['ent']:.2f} "
                f"| play={t_play:.1f}s upd={time.perf_counter()-t0-t_play:.1f}s ({time.perf_counter()-t0_all:.0f}s)",
                flush=True,
            )
            del res_list, trajs, batch, pl_all
            if it % 4 == 0:
                gc.collect()
                empty_cache(dev)
    finally:
        pool.close()
        pool.join()


if __name__ == "__main__":
    main()
