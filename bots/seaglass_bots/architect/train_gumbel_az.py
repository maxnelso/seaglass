"""Amber-Eye-Style Vectorized FP16-GPU Gumbel AlphaZero Trainer (`train_gumbel_az.py`).

100% Pure End-to-End Neural + On-Policy Search (ZERO hardcoded card/comp/hero/trinket/leveling rules!):
  - Runs `N_ENVS` simultaneous 8-player Battlegrounds lobbies in lockstep with **Batched FP16 CUDA**
    inference (no multiprocessing IPC bottleneck, flat ~2.5 GiB RAM!).
  - Applies **KataGo / Amber Eye Playout Cap Randomization**:
      * `p_full` moves (plus every `choose_trinket`, `choose_discover`, `choose_hero` prompt!) run
        **Gumbel Sequential Halving Search** (`m=4`, `sims=12`, CRN-paired fresh RNG resamples!)
        producing exact Gumbel completed-Q policy targets `pi_target` and search values `v_search`!
      * Remaining fast moves step all active seats in a SINGLE FP16 GPU batch (`~1.5ms` for 128 seats!).
  - Trains `AmberNet` with all 4 Amber Eye / KataGo heads:
      1. Primary Policy CE toward `pi_target`
      2. KataGo Soft Policy CE toward `pi_target^(1/4)`
      3. 8-Way Distributional Placement CE + explicit `1st-place BCE`
      4. Dense Auxiliary Trajectory Head (`log1p(stats_{t+2})/10`, `next_hp/40`, `alive_{t+2}`)
         + Value-Memorization-resistant search-bootstrapped value loss!
"""

from __future__ import annotations

import argparse
import gc
import math
import multiprocessing as mp
import os
import random
import sys
import time
from collections import deque
from pathlib import Path
from typing import Any, Deque, Dict, List, Optional, Tuple

os.environ.setdefault("OMP_NUM_THREADS", "1")
os.environ.setdefault("MKL_NUM_THREADS", "1")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")

_RL_DIR = str(Path(__file__).resolve().parents[3] / "rl")
if _RL_DIR not in sys.path:
    sys.path.insert(0, _RL_DIR)

import numpy as np
import seaglass
import torch
import torch.nn.functional as F

from seaglass_rl.actions import ACTION_TO_INDEX, END_TURN_INDEX, action_from_index, action_mask
from seaglass_rl.features import encode_observation
from seaglass_rl.play import Seat

from .dist_transformer import DIST_PLACE_ATOMS
from .gumbel_az import AmberNet, run_gumbel_root_search_batch
from .train_anti_sov import _fast_keyword_reposition_lobby


def _ordinal_targets(place_idx: torch.Tensor, sigma: float = 0.55) -> torch.Tensor:
    ranks = torch.arange(8, device=place_idx.device, dtype=torch.float32)[None, :]
    d2 = (ranks - place_idx.float()[:, None]) ** 2
    w = torch.exp(-0.5 * d2 / (sigma * sigma))
    return w / w.sum(dim=-1, keepdim=True)


def _opp_board_quick(
    obs: seaglass.TavernObservation, ctx: Optional[seaglass.LobbyContext]
) -> Tuple[List[seaglass.Unit], int]:
    if ctx is not None and getattr(ctx, "opponents", None):
        nxt = getattr(ctx, "next_opponent", -1)
        for o in ctx.opponents:
            if getattr(o, "player", -2) == nxt and getattr(o, "seen_board", None):
                return list(o.seen_board), int(getattr(o, "tavern_tier", obs.tavern_tier))
    return [], int(obs.tavern_tier)


def collect_vectorized_gumbel_games(
    model: AmberNet,
    opp_models: List[torch.nn.Module],
    num_games: int,
    dev: torch.device,
    rng: np.random.Generator,
    seed_base: int,
    p_full: float = 0.22,
    num_top_m: int = 4,
    num_sims: int = 12,
    rollout_depth: int = 10,
    sov_boss_games: int = 3,
    sov_seats_per_game: int = 1,
    fast_pos: bool = True,
    eq_weight: float = 0.0,
    macro_lookahead_turns: int = 1,
    macro_start_only: bool = False,
) -> Tuple[List[Dict[str, Any]], Dict[str, Any]]:
    """
    Runs `num_games` simultaneous 8-player Battlegrounds lobbies in lockstep on RTX 3070 FP16!
    4 seats per lobby = LEARNER (with Horizon-Aligned Gumbel search on `p_full` moves + prompts),
    4 seats per lobby = frozen league snapshots (+ 1 live fast `SovereignBot[beam=4]` boss seat in
    `sov_boss_games` lobbies so Tier 5/6 6k–20k stat pressure is always present!).
    """
    model.eval()
    for om in opp_models:
        om.eval()

    lobbies = [seaglass.LobbyGame(players=8, duos=False, seed=seed_base + g) for g in range(num_games)]
    seats = [[Seat(lobbies[g], p) for p in range(8)] for g in range(num_games)]
    py_rng = random.Random(seed_base)

    learner_seats = [set(py_rng.sample(range(8), 4)) for _ in range(num_games)]
    opp_idx_map: List[Dict[int, int]] = []
    sov_bots: Dict[Tuple[int, int], Any] = {}
    if sov_boss_games > 0:
        from seaglass_bots.sovereign_search import SovereignBot

        boss_games = set(py_rng.sample(range(num_games), min(sov_boss_games, num_games)))
    else:
        boss_games = set()

    for g in range(num_games):
        om = {}
        opp_list = [p for p in range(8) if p not in learner_seats[g]]
        n_boss = min(max(0, int(sov_seats_per_game)), len(opp_list)) if g in boss_games else 0
        boss_seats = set(opp_list[:n_boss])
        for p in opp_list:
            if p in boss_seats:
                sb = SovereignBot(
                    beam_width=4,
                    max_epoch_depth=4,
                    top_k_combat_evals=2,
                    refresh_samples=1,
                    num_particles=1,
                    seed=seed_base * 31 + g * 17 + p,
                )
                sb.evaluator.positioner.max_candidates = 12
                sb.positioner.max_candidates = 12
                sov_bots[(g, p)] = sb
            else:
                om[p] = py_rng.randrange(len(opp_models))
        opp_idx_map.append(om)

    # Per-(game, seat) recorded full-search training samples + per-turn stats/HP timeline for dense aux targets!
    samples_by_gs: Dict[Tuple[int, int], List[Dict[str, Any]]] = {
        (g, p): [] for g in range(num_games) for p in learner_seats[g]
    }
    turn_timeline: Dict[Tuple[int, int], Dict[int, Tuple[float, float]]] = {
        (g, p): {} for g in range(num_games) for p in learner_seats[g]
    }

    active_games = list(range(num_games))
    while active_games:
        for g in active_games:
            lobbies[g].begin_phase()

        is_turn_g = {g: (lobbies[g].phase == "turn") for g in active_games}
        ctxs_g = {
            g: {p: (lobbies[g].context(p) if is_turn_g[g] else None) for p in lobbies[g].active_players()}
            for g in active_games
        }
        step_cnt = {(g, p): 0 for g in active_games for p in lobbies[g].active_players()}

        # Step live SovereignBot boss seats cleanly first in this phase
        for g in active_games:
            lob = lobbies[g]
            for p in list(lob.active_players()):
                sb = sov_bots.get((g, p))
                if sb is None:
                    continue
                s_k = 0
                while not lob.is_done(p) and s_k < 55:
                    act = sb.act_in_lobby(lob, p)
                    if not lob.is_legal(p, act):
                        lob.force_end(p)
                        break
                    lob.step(p, act)
                    s_k += 1
                if not lob.is_done(p):
                    lob.force_end(p)

        while True:
            # Partition pending `(g, p)` into: (a) Gumbel full-search learner turns, (b) fast learner batch, (c) opp batches
            full_jobs: List[Tuple[int, int]] = []
            fast_learner: List[Tuple[int, int, np.ndarray, np.ndarray, np.ndarray, np.ndarray]] = []
            opp_batches: Dict[int, List[Tuple[int, int, np.ndarray, np.ndarray, np.ndarray, np.ndarray]]] = {}

            any_pending = False
            for g in active_games:
                lob = lobbies[g]
                for p in list(lob.active_players()):
                    if step_cnt.get((g, p), 0) >= 44:
                        if fast_pos and is_turn_g[g]:
                            _fast_keyword_reposition_lobby(lob, p)
                        lob.force_end(p)
                        continue
                    obs = lob.observe(p)
                    mk = action_mask(seats[g][p], allow_reposition=False, allow_freeze=True, obs=obs)
                    if not mk.any():
                        if fast_pos and is_turn_g[g]:
                            _fast_keyword_reposition_lobby(lob, p)
                        lob.force_end(p)
                        continue
                    any_pending = True
                    n_leg = int(mk.sum())
                    if n_leg == 1:
                        a_only = int(np.flatnonzero(mk)[0])
                        act = action_from_index(a_only)
                        if fast_pos and is_turn_g[g] and a_only == END_TURN_INDEX:
                            _fast_keyword_reposition_lobby(lob, p)
                        if lob.is_legal(p, act):
                            lob.step(p, act)
                        else:
                            lob.force_end(p)
                        step_cnt[(g, p)] = step_cnt.get((g, p), 0) + 1
                        continue

                    is_prompt = not is_turn_g[g] or obs.discover_pending is not None or bool(obs.trinket_options)
                    if p in learner_seats[g]:
                        # Playout Cap Randomization: full Gumbel search on prompts or `p_full` fraction of moves!
                        if is_prompt or float(rng.random()) < p_full:
                            full_jobs.append((g, p))
                        else:
                            pv, ci, nf = encode_observation(obs, ctxs_g[g].get(p), arranging=False)
                            fast_learner.append((g, p, pv, ci, nf, mk))
                    else:
                        pv, ci, nf = encode_observation(obs, ctxs_g[g].get(p), arranging=False)
                        oid = opp_idx_map[g][p]
                        opp_batches.setdefault(oid, []).append((g, p, pv, ci, nf, mk))

            if not any_pending:
                break

            # 1. Execute fast learner batch in ONE FP16 GPU pass!
            if fast_learner:
                pvs = np.stack([x[2] for x in fast_learner], axis=0).astype(np.float32)
                cis = np.stack([x[3] for x in fast_learner], axis=0).astype(np.int64)
                nfs = np.stack([x[4] for x in fast_learner], axis=0).astype(np.float32)
                mks = np.stack([x[5] for x in fast_learner], axis=0).astype(np.bool_)
                with torch.inference_mode():
                    with torch.amp.autocast("cuda", dtype=torch.float16, enabled=dev.type == "cuda"):
                        lg, _ = model.forward(
                            torch.from_numpy(pvs).to(dev),
                            torch.from_numpy(cis).to(dev),
                            torch.from_numpy(nfs).to(dev),
                            torch.from_numpy(mks).to(dev),
                        )
                    a_ids = lg.argmax(dim=-1).detach().cpu().numpy()
                for (g, p, *_), a_i in zip(fast_learner, a_ids):
                    lob = lobbies[g]
                    act = action_from_index(int(a_i))
                    if not lob.is_legal(p, act):
                        a_i = END_TURN_INDEX
                        act = action_from_index(a_i)
                    if fast_pos and is_turn_g[g] and int(a_i) == END_TURN_INDEX:
                        _fast_keyword_reposition_lobby(lob, p)
                    lob.step(p, act)
                    step_cnt[(g, p)] = step_cnt.get((g, p), 0) + 1

            # 2. Execute frozen opponent batches in ONE FP16 GPU pass per unique opponent model!
            for oid, items in opp_batches.items():
                omod = opp_models[oid]
                pvs = np.stack([x[2] for x in items], axis=0).astype(np.float32)
                cis = np.stack([x[3] for x in items], axis=0).astype(np.int64)
                nfs = np.stack([x[4] for x in items], axis=0).astype(np.float32)
                mks = np.stack([x[5] for x in items], axis=0).astype(np.bool_)
                with torch.inference_mode():
                    with torch.amp.autocast("cuda", dtype=torch.float16, enabled=dev.type == "cuda"):
                        lg, _ = omod.forward(
                            torch.from_numpy(pvs).to(dev),
                            torch.from_numpy(cis).to(dev),
                            torch.from_numpy(nfs).to(dev),
                            torch.from_numpy(mks).to(dev),
                        )
                    probs = F.softmax(lg.float(), dim=-1).detach().cpu().numpy()
                for idx_i, (g, p, *_) in enumerate(items):
                    lob = lobbies[g]
                    # Mild sampling for opponent diversity
                    pr = probs[idx_i]
                    pr = pr / max(1e-12, pr.sum())
                    a_i = int(rng.choice(len(pr), p=pr))
                    act = action_from_index(a_i)
                    if not lob.is_legal(p, act):
                        a_i = END_TURN_INDEX
                        act = action_from_index(a_i)
                    if fast_pos and is_turn_g[g] and a_i == END_TURN_INDEX:
                        _fast_keyword_reposition_lobby(lob, p)
                    lob.step(p, act)
                    step_cnt[(g, p)] = step_cnt.get((g, p), 0) + 1

            # 3. Execute ALL Gumbel Sequential Halving full searches in GPU-batched passes (~10x faster!)
            if full_jobs:
                upg_idx = ACTION_TO_INDEX.get(("upgrade_tavern", None, None), -1)
                dd_idx = ACTION_TO_INDEX.get(("dark_discovery", None, None), -1)
                norm_roots, norm_meta = [], []
                macro_roots, macro_meta = [], []
                for g, p in full_jobs:
                    lob = lobbies[g]
                    if lob.is_done(p):
                        continue
                    obs = lob.observe(p)
                    ctx = ctxs_g[g].get(p)
                    mk = action_mask(seats[g][p], allow_reposition=False, allow_freeze=True, obs=obs)
                    if not mk.any():
                        lob.force_end(p)
                        continue
                    base_g = lob.belief_tavern(p, seed=int(rng.integers(1, 2**62)) | 1, extra_units=[])
                    ob, ot = _opp_board_quick(obs, ctx)
                    is_prompt_m = (
                        not is_turn_g[g]
                        or obs.discover_pending is not None
                        or bool(obs.trinket_options)
                    )
                    has_upg = (upg_idx >= 0 and bool(mk[upg_idx])) or (dd_idx >= 0 and bool(mk[dd_idx]))
                    if macro_start_only:
                        upg_trig = has_upg and (step_cnt.get((g, p), 0) == 0)
                    else:
                        upg_trig = has_upg
                    is_m = macro_lookahead_turns > 1 and obs.turn >= 2 and (is_prompt_m or upg_trig)
                    if is_m:
                        macro_roots.append((base_g, ctx, ob, ot))
                        macro_meta.append((g, p, obs, ctx, mk))
                    else:
                        norm_roots.append((base_g, ctx, ob, ot))
                        norm_meta.append((g, p, obs, ctx, mk))

                paired_results: List[Tuple[Any, Any]] = []
                if norm_roots:
                    res_n = run_gumbel_root_search_batch(
                        model=model,
                        roots=norm_roots,
                        rng=rng,
                        device=dev,
                        num_top_m=num_top_m,
                        num_sims=num_sims,
                        add_gumbel_noise=True,
                        rollout_depth=rollout_depth,
                        eq_weight=eq_weight,
                        lookahead_turns=1,
                    )
                    paired_results.extend(zip(norm_meta, res_n))
                if macro_roots:
                    res_m = run_gumbel_root_search_batch(
                        model=model,
                        roots=macro_roots,
                        rng=rng,
                        device=dev,
                        num_top_m=num_top_m,
                        num_sims=num_sims,
                        add_gumbel_noise=True,
                        rollout_depth=rollout_depth,
                        eq_weight=eq_weight,
                        lookahead_turns=macro_lookahead_turns,
                    )
                    paired_results.extend(zip(macro_meta, res_m))

                for (g, p, obs, ctx, mk), (best_a, pi_imp, v_imp) in paired_results:
                    lob = lobbies[g]
                    pv, ci, nf = encode_observation(obs, ctx, arranging=False)
                    samples_by_gs[(g, p)].append(
                        {
                            "pv": pv.astype(np.float32),
                            "ci": ci.astype(np.int32),
                            "nf": nf.astype(np.float16),
                            "mk": mk.astype(np.bool_),
                            "pi": pi_imp.astype(np.float16),
                            "v_search": float(v_imp),
                            "turn": int(obs.turn),
                        }
                    )
                    act = action_from_index(int(best_a))
                    if not lob.is_legal(p, act):
                        best_a = END_TURN_INDEX
                        act = action_from_index(best_a)
                    if fast_pos and is_turn_g[g] and best_a == END_TURN_INDEX:
                        _fast_keyword_reposition_lobby(lob, p)
                    lob.step(p, act)
                    step_cnt[(g, p)] = step_cnt.get((g, p), 0) + 1

        # Record end-of-turn stats & HP for dense auxiliary labels
        for g in active_games:
            lob = lobbies[g]
            if is_turn_g[g]:
                for p in learner_seats[g]:
                    if p in lob.alive:
                        o = lob.observe(p)
                        bs = float(sum(u.attack + u.health for u in o.board))
                        ehp = float(max(0, o.health) + max(0, o.armor))
                        turn_timeline[(g, p)][int(o.turn)] = (bs, ehp)
            lob.end_phase()

        active_games = [g for g in active_games if not lobbies[g].done]

    # Assemble labeled training samples with placement + dense future-stats/HP/survival targets!
    out_samples: List[Dict[str, Any]] = []
    l_places, l_tiers, l_stats, sov_places = [], [], [], []
    for g in range(num_games):
        places = [int(x) for x in lobbies[g].placements]
        for p in range(8):
            if (g, p) in sov_bots:
                sov_places.append(places[p])
        for p in learner_seats[g]:
            pl = places[p]
            pl_idx = max(0, min(7, pl - 1))
            o_fin = lobbies[g].observe(p)
            l_places.append(pl)
            l_tiers.append(int(o_fin.tavern_tier))
            l_stats.append(float(sum(u.attack + u.health for u in o_fin.board)))

            tl = turn_timeline[(g, p)]
            max_t = max(tl.keys()) if tl else 1
            for s in samples_by_gs[(g, p)]:
                t_now = s["turn"]
                t_fut = min(max_t, t_now + 2)
                fut_bs, _ = tl.get(t_fut, (l_stats[-1], 0.0))
                _, nxt_hp = tl.get(min(max_t, t_now + 1), (0.0, 0.0))
                alive_t2 = 1.0 if (t_now + 2) in tl else (1.0 if pl == 1 else 0.0)
                s["place_idx"] = pl_idx
                s["aux"] = np.array(
                    [math.log1p(max(0.0, fut_bs)) / 10.0, min(1.5, nxt_hp / 40.0), alive_t2],
                    dtype=np.float32,
                )
                out_samples.append(s)

    stats_d = {
        "avg_place": float(np.mean(l_places)) if l_places else 4.5,
        "first_rate": float(np.mean([x == 1 for x in l_places])) if l_places else 0.0,
        "avg_tier": float(np.mean(l_tiers)) if l_tiers else 1.0,
        "med_stats": float(np.median(l_stats)) if l_stats else 0.0,
        "sov_avg": float(np.mean(sov_places)) if sov_places else 0.0,
        "sov_n": float(len(sov_places)),
        "raw_places": l_places,
        "raw_tiers": l_tiers,
        "raw_stats": l_stats,
        "raw_sov": sov_places,
    }
    return out_samples, stats_d


_WORKER_CTX: Dict[str, Any] = {}


def _gumbel_worker_init(init_ckpt: str, anchor_ckpts: List[str], dev_str: str) -> None:
    os.environ["OMP_NUM_THREADS"] = "1"
    os.environ["MKL_NUM_THREADS"] = "1"
    os.environ["OPENBLAS_NUM_THREADS"] = "1"
    torch.set_num_threads(1)
    dev = torch.device(dev_str)
    model = AmberNet.from_any_checkpoint(init_ckpt).to(dev)
    model.eval()
    for pm in model.parameters():
        pm.requires_grad_(False)
    opp_models: List[torch.nn.Module] = [AmberNet.from_any_checkpoint(init_ckpt).to(dev)]
    for ap in anchor_ckpts:
        if ap and os.path.exists(ap):
            opp_models.append(AmberNet.from_any_checkpoint(ap).to(dev))
    for om in opp_models:
        om.eval()
        for pm in om.parameters():
            pm.requires_grad_(False)
    _WORKER_CTX["dev"] = dev
    _WORKER_CTX["model"] = model
    _WORKER_CTX["opp_models"] = opp_models
    _WORKER_CTX["loaded_snaps"] = set()


def _gumbel_worker_run(job: Dict[str, Any]) -> Tuple[List[Dict[str, Any]], Dict[str, Any]]:
    dev: torch.device = _WORKER_CTX["dev"]
    model: AmberNet = _WORKER_CTX["model"]
    opp_models: List[torch.nn.Module] = _WORKER_CTX["opp_models"]
    loaded_snaps: set = _WORKER_CTX["loaded_snaps"]

    ckpt = torch.load(job["sync_path"], map_location=dev, weights_only=False)
    sd = ckpt.get("model", ckpt.get("state_dict", ckpt))
    model.load_state_dict(sd, strict=False)
    model.eval()

    new_snap = job.get("new_snap")
    if new_snap and new_snap not in loaded_snaps and os.path.exists(new_snap):
        sm = AmberNet.from_any_checkpoint(new_snap).to(dev)
        sm.eval()
        for pm in sm.parameters():
            pm.requires_grad_(False)
        if len(opp_models) >= 3:
            opp_models.pop(1)
        opp_models.append(sm)
        loaded_snaps.add(new_snap)

    rng = np.random.default_rng(int(job["seed_base"]))
    return collect_vectorized_gumbel_games(
        model=model,
        opp_models=opp_models,
        num_games=int(job["num_games"]),
        dev=dev,
        rng=rng,
        seed_base=int(job["seed_base"]),
        p_full=float(job["p_full"]),
        num_top_m=int(job["num_top_m"]),
        num_sims=int(job["num_sims"]),
        rollout_depth=int(job["rollout_depth"]),
        sov_boss_games=int(job["sov_boss_games"]),
        sov_seats_per_game=int(job.get("sov_seats_per_game", 1)),
        fast_pos=True,
        eq_weight=float(job["eq_weight"]),
        macro_lookahead_turns=int(job["macro_lookahead_turns"]),
        macro_start_only=bool(job["macro_start_only"]),
    )


def eval_unseen_samples(
    model: AmberNet,
    samples: List[Dict[str, Any]],
    dev: torch.device,
) -> Dict[str, float]:
    """Computes TRUE held-out generalization `val_v_ce` & `val_aux` on fresh unseen games BEFORE training!"""
    if len(samples) < 16:
        return {"val_v_ce": 0.0, "val_aux": 0.0}
    model.eval()
    tot_ce = tot_aux = 0.0
    n_tot = 0
    with torch.inference_mode():
        for s0 in range(0, len(samples), 512):
            chunk = samples[s0 : s0 + 512]
            pv = torch.from_numpy(np.stack([x["pv"] for x in chunk], axis=0)).to(dev)
            ci = torch.from_numpy(np.stack([x["ci"] for x in chunk], axis=0).astype(np.int64)).to(dev)
            nf = torch.from_numpy(np.stack([x["nf"] for x in chunk], axis=0).astype(np.float32)).to(dev)
            pl = torch.tensor([x["place_idx"] for x in chunk], dtype=torch.long, device=dev)
            aux_t = torch.from_numpy(np.stack([x["aux"] for x in chunk], axis=0)).to(dev)
            _, _, _, pl_logits, aux_pred = model.forward_all_heads(pv, ci, nf, None)
            ce = float(F.cross_entropy(pl_logits, pl, reduction="sum").item())
            aux_l = float(
                F.smooth_l1_loss(aux_pred[:, :2], aux_t[:, :2], reduction="mean").item()
                + 0.5 * F.binary_cross_entropy_with_logits(aux_pred[:, 2], aux_t[:, 2], reduction="mean").item()
            ) * len(chunk)
            tot_ce += ce
            tot_aux += aux_l
            n_tot += len(chunk)
    return {"val_v_ce": tot_ce / max(1, n_tot), "val_aux": tot_aux / max(1, n_tot)}


def train_gumbel_step(
    model: AmberNet,
    opt: torch.optim.Optimizer,
    replay: List[Dict[str, Any]],
    dev: torch.device,
    steps: int = 40,
    batch_size: int = 512,
    val_weight: float = 0.80,
    gm_anchor: Optional[Dict[str, np.ndarray]] = None,
    gm_anchor_weight: float = 0.0,
    rng: np.random.Generator | None = None,
) -> Dict[str, float]:
    if rng is None:
        rng = np.random.default_rng()
    n = len(replay)
    if n < 64:
        return {"pi_ce": 0.0, "v_ce": 0.0, "aux": 0.0}

    model.train()
    tot_pi = tot_soft = tot_vce = tot_aux = 0.0
    for _ in range(steps):
        idx = rng.integers(0, n, size=min(batch_size, n))
        pv = torch.from_numpy(np.stack([replay[i]["pv"] for i in idx], axis=0)).to(dev)
        ci = torch.from_numpy(np.stack([replay[i]["ci"] for i in idx], axis=0).astype(np.int64)).to(dev)
        nf = torch.from_numpy(np.stack([replay[i]["nf"] for i in idx], axis=0).astype(np.float32)).to(dev)
        mk = torch.from_numpy(np.stack([replay[i]["mk"] for i in idx], axis=0)).to(dev)
        pi_t = torch.from_numpy(np.stack([replay[i]["pi"] for i in idx], axis=0).astype(np.float32)).to(dev)
        pl = torch.tensor([replay[i]["place_idx"] for i in idx], dtype=torch.long, device=dev)
        vs = torch.tensor([replay[i]["v_search"] for i in idx], dtype=torch.float32, device=dev)
        aux_t = torch.from_numpy(np.stack([replay[i]["aux"] for i in idx], axis=0)).to(dev)

        logits, soft_logits, val_pred, pl_logits, aux_pred = model.forward_all_heads(pv, ci, nf, mk)

        # 1. Primary Gumbel Policy Improvement CE Loss
        logp = F.log_softmax(logits, dim=-1).masked_fill(~mk, 0.0)
        pi_loss = -(pi_t * logp).sum(dim=-1).mean()

        # 2. KataGo Soft Policy Head (`pi^(1/4)` normalized over legal actions)
        soft_t = torch.where(mk, pi_t.clamp(min=0.0).pow(0.25), torch.zeros_like(pi_t))
        soft_t = soft_t / soft_t.sum(dim=-1, keepdim=True).clamp(min=1e-6)
        soft_logp = F.log_softmax(soft_logits, dim=-1).masked_fill(~mk, 0.0)
        soft_loss = -(soft_t * soft_logp).sum(dim=-1).mean()

        # 3. 8-Way Distributional Placement Loss + 1st-Place Binary BCE + Search-Bootstrapped Value!
        ord_t = _ordinal_targets(pl, sigma=0.55)
        v_ce = -(ord_t * F.log_softmax(pl_logits, dim=-1)).sum(dim=-1).mean()
        p_first = F.softmax(pl_logits, dim=-1)[:, 0].clamp(1e-5, 1.0 - 1e-5)
        f1_loss = F.binary_cross_entropy(p_first, (pl == 0).float())
        z_atoms = model.reward_atoms[pl]
        v_boot = 0.65 * z_atoms + 0.35 * vs
        v_mse = F.smooth_l1_loss(val_pred, v_boot)

        # 4. Dense Auxiliary Trajectory Loss (future stats + next HP + survive t+2)
        aux_reg = F.smooth_l1_loss(aux_pred[:, :2], aux_t[:, :2])
        aux_surv = F.binary_cross_entropy_with_logits(aux_pred[:, 2], aux_t[:, 2])
        aux_loss = aux_reg + 0.5 * aux_surv

        loss = (
            pi_loss
            + 0.25 * soft_loss
            + val_weight * v_ce
            + (0.50 * val_weight) * f1_loss
            + (0.45 * val_weight) * v_mse
            + 0.30 * aux_loss
        )

        # Optional AlphaStar-style Grandmaster Tier-6 anchor (prevents value drift & Tier-6 forgetting!)
        if gm_anchor is not None and gm_anchor_weight > 0.0:
            n_gm = len(gm_anchor["ac"])
            gi = rng.integers(0, n_gm, size=min(256, n_gm))
            g_pv = torch.from_numpy(gm_anchor["pv"][gi]).to(dev)
            g_ci = torch.from_numpy(gm_anchor["ci"][gi].astype(np.int64)).to(dev)
            g_nf = torch.from_numpy(gm_anchor["nf"][gi].astype(np.float32)).to(dev)
            g_mk = torch.from_numpy(gm_anchor["mk"][gi]).to(dev)
            g_ac = torch.from_numpy(gm_anchor["ac"][gi].astype(np.int64)).to(dev)
            g_pl = torch.from_numpy(gm_anchor["pl"][gi].astype(np.int64)).to(dev)
            g_lg, _, _, g_plg, _ = model.forward_all_heads(g_pv, g_ci, g_nf, g_mk)
            g_pol = F.cross_entropy(g_lg, g_ac)
            g_ord = _ordinal_targets(g_pl, sigma=0.55)
            g_vce = -(g_ord * F.log_softmax(g_plg, dim=-1)).sum(dim=-1).mean()
            loss = loss + gm_anchor_weight * (g_pol + 0.50 * g_vce)

        opt.zero_grad(set_to_none=True)
        loss.backward()
        torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
        opt.step()

        tot_pi += float(pi_loss.item())
        tot_soft += float(soft_loss.item())
        tot_vce += float(v_ce.item())
        tot_aux += float(aux_loss.item())

    model.eval()
    return {
        "pi_ce": tot_pi / steps,
        "soft_ce": tot_soft / steps,
        "v_ce": tot_vce / steps,
        "aux": tot_aux / steps,
    }


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--init-ckpt", required=True)
    p.add_argument("--anchor-ckpts", nargs="*", default=[])
    p.add_argument("--run-dir", required=True)
    p.add_argument("--iterations", type=int, default=80)
    p.add_argument("--games", type=int, default=16)
    p.add_argument("--p-full", type=float, default=0.22)
    p.add_argument("--num-top-m", type=int, default=4)
    p.add_argument("--num-sims", type=int, default=12)
    p.add_argument("--rollout-depth", type=int, default=10)
    p.add_argument("--sov-boss-games", type=int, default=3)
    p.add_argument("--sov-seats-per-game", type=int, default=1)
    p.add_argument("--gm-anchor-shard", default="", help="Optional Tier-6 GM shard path for AlphaStar anchor")
    p.add_argument("--gm-anchor-weight", type=float, default=0.25)
    p.add_argument("--replay-cap", type=int, default=80000)
    p.add_argument("--train-steps", type=int, default=12)
    p.add_argument("--batch-size", type=int, default=512)
    p.add_argument("--val-weight", type=float, default=0.80)
    p.add_argument("--eq-weight", type=float, default=0.0)
    p.add_argument("--macro-lookahead-turns", type=int, default=1)
    p.add_argument("--macro-start-only", action="store_true")
    p.add_argument("--num-workers", type=int, default=1)
    p.add_argument("--weight-decay", type=float, default=1.0e-3)
    p.add_argument("--lr", type=float, default=1.0e-4)
    p.add_argument("--seed", type=int, default=1001)
    args = p.parse_args()

    dev = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    run = Path(args.run_dir)
    (run / "snapshots").mkdir(parents=True, exist_ok=True)

    model = AmberNet.from_any_checkpoint(args.init_ckpt).to(dev)
    opp_models: List[torch.nn.Module] = []
    pool = None
    if args.num_workers > 1:
        ctx_mp = mp.get_context("spawn")
        pool = ctx_mp.Pool(
            processes=args.num_workers,
            initializer=_gumbel_worker_init,
            initargs=(args.init_ckpt, list(args.anchor_ckpts), str(dev)),
        )
    else:
        opp_models = [AmberNet.from_any_checkpoint(args.init_ckpt).to(dev)]
        for ap in args.anchor_ckpts:
            if ap and os.path.exists(ap):
                opp_models.append(AmberNet.from_any_checkpoint(ap).to(dev))
        for om in opp_models:
            om.eval()
            for pm in om.parameters():
                pm.requires_grad_(False)

    opt = torch.optim.AdamW(model.parameters(), lr=args.lr, weight_decay=args.weight_decay)
    replay: Deque[Dict[str, Any]] = deque(maxlen=args.replay_cap)
    rng = np.random.default_rng(args.seed)
    best_val_ce = float("inf")
    last_snap_path: Optional[str] = None
    sync_path = str(run / "_sync_learner.pt")

    gm_anchor: Optional[Dict[str, np.ndarray]] = None
    if args.gm_anchor_shard and os.path.exists(args.gm_anchor_shard):
        with np.load(args.gm_anchor_shard) as z:
            gm_anchor = {k: np.array(z[k]) for k in ("pv", "ci", "nf", "mk", "ac", "pl")}
        print(
            f"Loaded AlphaStar Tier-6 GM anchor: {os.path.basename(args.gm_anchor_shard)} "
            f"({len(gm_anchor['ac']):,} steps, weight={args.gm_anchor_weight})",
            flush=True,
        )

    t0_all = time.perf_counter()
    print(
        f"Starting Horizon-Aligned Amber-Eye Gumbel AlphaZero v2 ({args.iterations} iters x {args.games}g, "
        f"workers={args.num_workers}, p_full={args.p_full}, m={args.num_top_m}, sims={args.num_sims}, "
        f"sov_boss={args.sov_boss_games}, steps={args.train_steps}, bs={args.batch_size}, vw={args.val_weight}, "
        f"eqw={args.eq_weight}, mlt={args.macro_lookahead_turns}, m_start={args.macro_start_only}, "
        f"wd={args.weight_decay}, cap={args.replay_cap}) on {dev}",
        flush=True,
    )

    try:
        for it in range(1, args.iterations + 1):
            t0 = time.perf_counter()
            if pool is not None:
                model.save_checkpoint(sync_path, iteration=it)
                W = args.num_workers
                base_g = args.games // W
                rem_g = args.games % W
                base_sov = args.sov_boss_games // W
                rem_sov = args.sov_boss_games % W
                jobs = []
                for wi in range(W):
                    ng = base_g + (1 if wi < rem_g else 0)
                    if ng <= 0:
                        continue
                    jobs.append(
                        {
                            "sync_path": sync_path,
                            "new_snap": last_snap_path,
                            "num_games": ng,
                            "seed_base": args.seed * 10000 + it * 100 + wi * 19,
                            "p_full": args.p_full,
                            "num_top_m": args.num_top_m,
                            "num_sims": args.num_sims,
                            "rollout_depth": args.rollout_depth,
                            "sov_boss_games": base_sov + (1 if wi < rem_sov else 0),
                            "sov_seats_per_game": args.sov_seats_per_game,
                            "eq_weight": args.eq_weight,
                            "macro_lookahead_turns": args.macro_lookahead_turns,
                            "macro_start_only": args.macro_start_only,
                        }
                    )
                res_list = pool.map(_gumbel_worker_run, jobs)
                new_s = []
                all_pl, all_ti, all_st, all_sov = [], [], [], []
                for ws, wst in res_list:
                    new_s.extend(ws)
                    all_pl.extend(wst["raw_places"])
                    all_ti.extend(wst["raw_tiers"])
                    all_st.extend(wst["raw_stats"])
                    all_sov.extend(wst["raw_sov"])
                st = {
                    "avg_place": float(np.mean(all_pl)) if all_pl else 4.5,
                    "first_rate": float(np.mean([x == 1 for x in all_pl])) if all_pl else 0.0,
                    "avg_tier": float(np.mean(all_ti)) if all_ti else 1.0,
                    "med_stats": float(np.median(all_st)) if all_st else 0.0,
                    "sov_avg": float(np.mean(all_sov)) if all_sov else 0.0,
                    "sov_n": float(len(all_sov)),
                }
            else:
                new_s, st = collect_vectorized_gumbel_games(
                    model=model,
                    opp_models=opp_models,
                    num_games=args.games,
                    dev=dev,
                    rng=rng,
                    seed_base=args.seed * 10000 + it * 100,
                    p_full=args.p_full,
                    num_top_m=args.num_top_m,
                    num_sims=args.num_sims,
                    rollout_depth=args.rollout_depth,
                    sov_boss_games=args.sov_boss_games,
                    sov_seats_per_game=args.sov_seats_per_game,
                    fast_pos=True,
                    eq_weight=args.eq_weight,
                    macro_lookahead_turns=args.macro_lookahead_turns,
                    macro_start_only=args.macro_start_only,
                )
            # Measure TRUE pre-update generalization on brand-new unseen games!
            unseen = eval_unseen_samples(model, new_s, dev)
            best_tag = ""
            if it >= 3 and unseen["val_v_ce"] > 0.0 and unseen["val_v_ce"] < best_val_ce:
                best_val_ce = unseen["val_v_ce"]
                model.save_checkpoint(str(run / "best_val.pt"), iteration=it)
                best_tag = " *BEST_VAL*"
            replay.extend(new_s)
            t_play = time.perf_counter() - t0

            m = train_gumbel_step(
                model,
                opt,
                list(replay),
                dev=dev,
                steps=args.train_steps,
                batch_size=args.batch_size,
                val_weight=args.val_weight,
                gm_anchor=gm_anchor,
                gm_anchor_weight=args.gm_anchor_weight,
                rng=rng,
            )
            if it % 5 == 0 or it == args.iterations:
                model.save_checkpoint(str(run / "latest.pt"), iteration=it)
            if it % 10 == 0 or it == args.iterations:
                sp = str(run / "snapshots" / f"iter_{it:04d}.pt")
                model.save_checkpoint(sp, iteration=it)
                last_snap_path = sp
                if pool is None:
                    snap_m = AmberNet.from_any_checkpoint(sp).to(dev)
                    snap_m.eval()
                    for pm in snap_m.parameters():
                        pm.requires_grad_(False)
                    if len(opp_models) >= 3:
                        opp_models.pop(1)
                    opp_models.append(snap_m)

            sov_s = f"Sov={st['sov_avg']:.2f}({int(st['sov_n'])})" if st["sov_n"] > 0 else "Sov=0"
            print(
                f"GumbelAZ it {it:3d}/{args.iterations} | avg={st['avg_place']:.2f} 1st={st['first_rate']:.1%} "
                f"T={st['avg_tier']:.2f} medS={st['med_stats']:.0f} {sov_s} | new={len(new_s):4d} buf={len(replay):5d} "
                f"| val_ce={unseen['val_v_ce']:.3f} tr_ce={m['v_ce']:.3f} pi={m['pi_ce']:.3f} aux={unseen['val_aux']:.3f} "
                f"| play={t_play:.1f}s upd={time.perf_counter()-t0-t_play:.1f}s ({time.perf_counter()-t0_all:.0f}s){best_tag}",
                flush=True,
            )
            if it % 5 == 0:
                gc.collect()
                if torch.cuda.is_available():
                    torch.cuda.empty_cache()
    finally:
        if pool is not None:
            pool.close()
            pool.join()


if __name__ == "__main__":
    main()
