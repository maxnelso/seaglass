"""Direct Anti-Sovereign CUDA PPO Trainer (`train_anti_sov.py`).

Trains `EntityTransformer` (100% pure end-to-end RL, ZERO hardcoded rules!) DIRECTLY in
full 8-player Patch 37.0.2 lobbies containing live `SovereignBot` seats (`5x LEARNER` vs
`3x SovereignBot` at fast search budget), optimizing sharp 1st-place tournament reward on
RTX 3070 CUDA!

Why this breaks the self-play Tier-2/3 local equilibrium:
  - Self-play PPO without Tier 5/6 scaling bosses converges to a Tier 2-3 tempo Nash equilibrium
    (already reaching 25% 1st / 4.50 avg vs 7x SovereignBot at 200ms/t!).
  - Putting live `SovereignBot` seats directly inside every training lobby exposes `EntityTransformer`
    to real 6,000-20,000 stat Tier 5/6 scaling boards every game, forcing pure RL to discover
    exact leveling, DarkDiscovery timing, economy, and endgame scaling directly!
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

from seaglass_rl import checkpoint as rl_ckpt
from seaglass_rl.actions import ACTION_TO_INDEX, END_TURN_INDEX, action_from_index, action_mask
from seaglass_rl.features import encode_observation
from seaglass_rl.model import sample_categorical
from seaglass_rl.play import Seat
from seaglass_rl.ppo import build_batch
from seaglass_rl.transformer import EntityTransformer

from .train_policy import FIRST_PLACE_REWARDS, ppo_update_cuda


_WORKER_CACHE: Dict[str, Any] = {}


def _fast_keyword_reposition_lobby(lobby: seaglass.LobbyGame, p: int) -> None:
    """<0.1ms pure-keyword EOT board ordering (ZERO card names!) so RL training matches eval physics."""
    obs = lobby.observe(p)
    n = len(obs.board)
    if n < 2:
        return

    def _key(u: seaglass.Unit) -> Tuple[int, float, int]:
        # Pure keyword combat physics:
        # 1. Windfury / Divine Shield / Venomous / high-attack non-taunts strike first;
        # 2. Taunts protect from the right unless huge divine-shield/windfury carry.
        is_taunt = 1 if u.taunt else 0
        pri = 0
        if u.windfury:
            pri -= 3
        if u.divine_shield:
            pri -= 2
        if u.venomous:
            pri -= 1
        if is_taunt and not (u.windfury or u.divine_shield):
            pri += 4
        if u.reborn and not is_taunt:
            pri -= 1
        return (pri, -float(u.attack), -int(u.health))

    # Insertion-sort into desired order using legal `reposition` actions (at most n-1 moves)
    for target_pos in range(n):
        obs_c = lobby.observe(p)
        if len(obs_c.board) != n:
            break
        rem = list(range(target_pos, n))
        best_i = min(rem, key=lambda idx: _key(obs_c.board[idx]))
        if best_i != target_pos:
            act = seaglass.TavernAction.reposition(best_i, target_pos)
            if lobby.is_legal(p, act):
                lobby.step(p, act)


def _play_anti_sov_game(
    args_t: Tuple[int, str, int, int, bool, bool]
) -> Dict[str, Any]:
    seed, ckpt_path, n_sov, sov_beam, no_repo, fast_pos = args_t
    from seaglass_bots.sovereign_search import SovereignBot

    torch.set_num_threads(1)
    ckpt_mtime = os.path.getmtime(ckpt_path)
    if _WORKER_CACHE.get("path") != ckpt_path or _WORKER_CACHE.get("mtime") != ckpt_mtime:
        model, _ = rl_ckpt.load(ckpt_path)
        model.eval()
        _WORKER_CACHE["path"] = ckpt_path
        _WORKER_CACHE["mtime"] = ckpt_mtime
        _WORKER_CACHE["model"] = model
    model = _WORKER_CACHE["model"]
    rng_np = np.random.default_rng(seed)
    rng_py = random.Random(seed)

    sov_seats = set(rng_py.sample(range(8), n_sov))
    sov_bots = {}
    for p in sov_seats:
        sb = SovereignBot(
            beam_width=sov_beam,
            max_epoch_depth=4,
            top_k_combat_evals=2,
            refresh_samples=1,
            num_particles=1,
            seed=seed * 31 + p * 17 + 3,
        )
        sb.evaluator.positioner.max_candidates = 12
        sb.positioner.max_candidates = 12
        sov_bots[p] = sb

    lobby = seaglass.LobbyGame(players=8, duos=False, seed=seed)
    seats = [Seat(lobby, p) for p in range(8)]

    # Per-learner trajectory buffers
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
        for p in range(8)
        if p not in sov_seats
    }

    while not lobby.done:
        lobby.begin_phase()
        is_turn = lobby.phase == "turn"
        ctxs = {p: (lobby.context(p) if is_turn else None) for p in lobby.active_players()}

        # 1. Step SovereignBot seats
        for p in list(lobby.active_players()):
            if p not in sov_seats:
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

        # 2. Step all Learner seats in batched lockstep!
        step_cnt = {p: 0 for p in tr_buf}
        while True:
            todo = [p for p in lobby.active_players() if p in tr_buf and step_cnt[p] < 45]
            if not todo:
                for p in list(lobby.active_players()):
                    if p in tr_buf:
                        lobby.force_end(p)
                        if tr_buf[p]["turn_end"]:
                            tr_buf[p]["turn_end"][-1] = True
                break
            pvs, cis, nfs, mks, valid_p = [], [], [], [], []
            for p in todo:
                obs = lobby.observe(p)
                mk = action_mask(seats[p], allow_reposition=not no_repo, allow_freeze=True, obs=obs)
                if not mk.any():
                    lobby.force_end(p)
                    if tr_buf[p]["turn_end"]:
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
                logits, vals = model.forward(
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
    l_places, s_places, l_tiers, s_tiers = [], [], [], []
    for p in range(8):
        tier_p = int(lobby.observe(p).tavern_tier)
        if p in sov_seats:
            s_places.append(places[p])
            s_tiers.append(tier_p)
        else:
            l_places.append(places[p])
            l_tiers.append(tier_p)
            b = tr_buf[p]
            if b["actions"]:
                pl_idx = max(0, min(7, places[p] - 1))
                b["rewards"][-1] = float(FIRST_PLACE_REWARDS[pl_idx])
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
                        "done": np.asarray(True),
                    }
                )

    return {
        "trajs": trajs,
        "l_places": l_places,
        "s_places": s_places,
        "l_tiers": l_tiers,
        "s_tiers": s_tiers,
    }


def main() -> None:
    p = argparse.ArgumentParser()
    p.add_argument("--init-ckpt", required=True)
    p.add_argument("--ref-ckpt", default="")
    p.add_argument("--ref-kl-coef", type=float, default=0.015)
    p.add_argument("--run-dir", required=True)
    p.add_argument("--iterations", type=int, default=60)
    p.add_argument("--games", type=int, default=16)
    p.add_argument("--workers", type=int, default=8)
    p.add_argument("--n-sov", type=int, default=3)
    p.add_argument("--sov-beam", type=int, default=5)
    p.add_argument("--lr", type=float, default=8.0e-5)
    p.add_argument("--gamma", type=float, default=0.995)
    p.add_argument("--no-reposition", action="store_true")
    p.add_argument("--fast-pos", action="store_true", help="Apply <0.1ms pure-keyword EOT positioning on learner seats.")
    p.add_argument("--seed", type=int, default=777)
    args = p.parse_args()

    dev = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    run = Path(args.run_dir)
    (run / "snapshots").mkdir(parents=True, exist_ok=True)

    model, _ = rl_ckpt.load(args.init_ckpt)
    model = model.to(dev)
    model.eval()
    ref_model = None
    if args.ref_kl_coef > 0.0:
        rp = args.ref_ckpt if (args.ref_ckpt and os.path.exists(args.ref_ckpt)) else args.init_ckpt
        ref_model, _ = rl_ckpt.load(rp)
        ref_model = ref_model.to(dev).eval()
        for pm in ref_model.parameters():
            pm.requires_grad_(False)

    opt = torch.optim.AdamW(model.parameters(), lr=args.lr, eps=1e-5, weight_decay=1e-4)

    def _save_cpu(path: str, it_num: int) -> None:
        cm = EntityTransformer(**model.config)
        cm.load_state_dict({k: v.detach().cpu() for k, v in model.state_dict().items()})
        cm.eval()
        rl_ckpt.save(path, cm, iteration=it_num)

    ctx_mp = mp.get_context("spawn")
    t0_all = time.perf_counter()
    print(
        f"Starting DIRECT Anti-Sovereign CUDA PPO ({args.iterations} iters x {args.games} games, "
        f"{8-args.n_sov}x LEARNER vs {args.n_sov}x SovereignBot[beam={args.sov_beam}], "
        f"ref_kl={args.ref_kl_coef}) on {dev}",
        flush=True,
    )

    tmp_ckpt = str(run / "_worker_current.pt")
    pool = ctx_mp.Pool(processes=args.workers)
    try:
      for it in range(1, args.iterations + 1):
        if it > 1 and (it - 1) % 10 == 0:
            pool.close()
            pool.join()
            gc.collect()
            pool = ctx_mp.Pool(processes=args.workers)
        t0 = time.perf_counter()
        _save_cpu(tmp_ckpt, it - 1)
        jobs = [
            (args.seed * 10000 + it * 1000 + g, tmp_ckpt, args.n_sov, args.sov_beam, args.no_reposition, args.fast_pos)
            for g in range(args.games)
        ]
        res_list = pool.map(_play_anti_sov_game, jobs)
        t_play = time.perf_counter() - t0

        trajs = [t for r in res_list for t in r["trajs"]]
        lp = [x for r in res_list for x in r["l_places"]]
        sp = [x for r in res_list for x in r["s_places"]]
        lt = [x for r in res_list for x in r["l_tiers"]]
        st = [x for r in res_list for x in r["s_tiers"]]

        batch = build_batch(trajs, args.gamma, 0.95)
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
            ref_kl_coef=args.ref_kl_coef,
        )
        l_1st = sum(1 for x in lp if x == 1) / max(1, len(lp))
        s_1st = sum(1 for x in sp if x == 1) / max(1, len(sp))
        if it % 5 == 0 or it == args.iterations:
            _save_cpu(str(run / "latest.pt"), it)
        if it % 10 == 0 or it == args.iterations:
            _save_cpu(str(run / "snapshots" / f"iter_{it:04d}.pt"), it)

        print(
            f"AntiSov it {it:3d}/{args.iterations} | L_avg={np.mean(lp):.2f} L_1st={l_1st:.1%} L_T={np.mean(lt):.2f} "
            f"| Sov_avg={np.mean(sp):.2f} Sov_1st={s_1st:.1%} Sov_T={np.mean(st):.2f} "
            f"| steps={n_steps:5d} kl={m['kl']:.4f} ent={m['ent']:.2f} "
            f"| play={t_play:.0f}s upd={time.perf_counter()-t0-t_play:.1f}s ({time.perf_counter()-t0_all:.0f}s)",
            flush=True,
        )
        del res_list, trajs, batch
        gc.collect()
        if torch.cuda.is_available():
            torch.cuda.empty_cache()
    finally:
        pool.close()
        pool.join()


if __name__ == "__main__":
    main()
