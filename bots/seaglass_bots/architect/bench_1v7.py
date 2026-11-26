"""Parallel 1v7 FFA Benchmark Harness: 1 Challenger vs 7 SovereignBots across N games."""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from concurrent.futures import ProcessPoolExecutor, as_completed
from typing import Any, Dict, List, Tuple

# Ensure single-threaded BLAS per worker process to prevent thread thrashing across cores!
os.environ.setdefault("OMP_NUM_THREADS", "1")
os.environ.setdefault("MKL_NUM_THREADS", "1")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")
os.environ.setdefault("NUMEXPR_NUM_THREADS", "1")

import seaglass


def _make_bot(kind: str, seed: int, kwargs: Dict[str, Any]):
    from seaglass_bots import ApexBot, ArchitectBot, SovereignBot, TempoBot
    from seaglass_bots.architect.alpha_ppo_bot import PPOAlphaSearchBot
    from seaglass_bots.architect.dag_search_bot import DistDAGSearchBot
    from seaglass_bots.architect.grandmaster import GrandmasterBot
    from seaglass_bots.architect.gumbel_az import GumbelSearchBot
    from seaglass_bots.architect.neural_beam_bot import NeuralPhysicsBeamBot
    from seaglass_bots.architect.oracle_zero_bot import OracleZeroBot
    from seaglass_bots.architect.policy_beam_bot import PolicyGuidedBeamBot
    from seaglass_bots.architect.policy_bot import NeuralPolicyBot, PolicyZeroBot
    from seaglass_bots.architect.rl_physics_bot import FinalsPhysicsRLBot
    from seaglass_bots.architect.scaling_search_bot import ScalingSearchBot
    from seaglass_bots.architect.zero_bot import ZeroPhysicsBot

    if kind == "policy_beam":
        return PolicyGuidedBeamBot(seed=seed, **kwargs)
    if kind == "scaling_search":
        return ScalingSearchBot(seed=seed, **kwargs)
    if kind == "neural_beam":
        return NeuralPhysicsBeamBot(seed=seed, **kwargs)
    if kind == "dag_search":
        return DistDAGSearchBot(seed=seed, **kwargs)
    if kind == "gumbel_az":
        return GumbelSearchBot(seed=seed, **kwargs)
    if kind == "rl_physics":
        return FinalsPhysicsRLBot(seed=seed, **kwargs)
    if kind == "oracle_zero":
        return OracleZeroBot(seed=seed, **kwargs)
    if kind == "alpha_ppo":
        return PPOAlphaSearchBot(seed=seed, **kwargs)
    if kind == "policy":
        return NeuralPolicyBot(seed=seed, **kwargs)
    if kind == "policy_zero":
        return PolicyZeroBot(seed=seed, **kwargs)
    if kind == "zero":
        return ZeroPhysicsBot(seed=seed, **kwargs)
    if kind == "grandmaster":
        return GrandmasterBot(seed=seed, **kwargs)
    if kind == "architect":
        return ArchitectBot(seed=seed, **kwargs)
    if kind == "sovereign":
        return SovereignBot(seed=seed, **kwargs)
    if kind == "apex":
        return ApexBot(seed=seed, **kwargs)
    if kind == "tempo":
        return TempoBot(seed=seed)
    raise ValueError(f"Unknown bot kind: {kind}")


def _run_one_game(args: Tuple[int, int, str, Dict[str, Any], str, str | None]) -> Dict[str, Any]:
    game_idx, seed, challenger_kind, challenger_kwargs, opp_kind, save_dir = args
    seat = game_idx % 8  # Rotate challenger seat evenly across 0..7!
    bots = {}
    names = []
    for p in range(8):
        s_seed = seed + p * 37 + 11
        if p == seat:
            bots[p] = _make_bot(challenger_kind, s_seed, challenger_kwargs)
            names.append(f"{challenger_kind}_p{p}")
        else:
            bots[p] = _make_bot(opp_kind, s_seed, {})
            names.append(f"{opp_kind}_p{p}")

    lobby = seaglass.LobbyGame(players=8, duos=False, seed=seed, names=names)
    t_game0 = time.perf_counter()
    challenger_ms: List[float] = []
    opp_ms: List[float] = []
    turn_log: List[Dict[str, Any]] = []

    while not lobby.done:
        lobby.begin_phase()
        is_turn = lobby.phase == "turn"
        for p in list(lobby.active_players()):
            t0 = time.perf_counter() if is_turn else 0.0
            steps = 0
            while not lobby.is_done(p) and steps < 80:
                act = bots[p].act_in_lobby(lobby, p)
                if not lobby.is_legal(p, act):
                    lobby.force_end(p)
                    break
                note = getattr(bots[p], "last_note", None)
                lobby.step(p, act, note=note)
                steps += 1
            if not lobby.is_done(p):
                lobby.force_end(p)
            if is_turn:
                dt = (time.perf_counter() - t0) * 1000.0
                if p == seat:
                    challenger_ms.append(dt)
                else:
                    opp_ms.append(dt)
        if is_turn and lobby.alive[seat]:
            obs = lobby.observe(seat)
            turn_log.append(
                {
                    "turn": obs.turn,
                    "hp": obs.health + obs.armor,
                    "tier": obs.tavern_tier,
                    "stats": sum(u.attack + u.health for u in obs.board),
                    "comp": getattr(getattr(bots[seat], "_last_plan", None), "primary_id", ""),
                }
            )
        lobby.end_phase()

    places = list(lobby.placements)
    c_place = places[seat]
    c_obs = lobby.observe(seat)
    winner_seat = places.index(1) if 1 in places else 0
    w_obs = lobby.observe(winner_seat)

    if save_dir:
        os.makedirs(save_dir, exist_ok=True)
        out_path = os.path.join(
            save_dir, f"g{game_idx:03d}_seed{seed}_seat{seat}_place{c_place}_t{lobby.turn}.replay.pb"
        )
        lobby.set_meta("bot", challenger_kind)
        lobby.save(out_path)

    return {
        "game_idx": game_idx,
        "seed": seed,
        "seat": seat,
        "place": c_place,
        "turns": lobby.turn,
        "tribes": list(lobby.tribes),
        "c_tier": c_obs.tavern_tier,
        "c_hero": c_obs.hero_id,
        "c_comp": getattr(getattr(bots[seat], "_last_plan", None), "primary_id", ""),
        "c_board": [(u.name, f"{u.attack}/{u.health}", u.is_golden) for u in c_obs.board],
        "w_seat": winner_seat,
        "w_is_challenger": winner_seat == seat,
        "w_tier": w_obs.tavern_tier,
        "w_board": [(u.name, f"{u.attack}/{u.health}", u.is_golden) for u in w_obs.board],
        "c_ms": sum(challenger_ms) / max(1, len(challenger_ms)),
        "opp_ms": sum(opp_ms) / max(1, len(opp_ms)),
        "wall_s": time.perf_counter() - t_game0,
        "turn_log": turn_log,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--challenger", default="architect")
    parser.add_argument("--opponent", default="sovereign")
    parser.add_argument("--games", type=int, default=16)
    parser.add_argument("--start-game", type=int, default=0)
    parser.add_argument("--workers", type=int, default=14)
    parser.add_argument("--base-seed", type=int, default=20000)
    parser.add_argument("--kwargs", type=str, default="{}")
    parser.add_argument("--save-dir", type=str, default=None)
    parser.add_argument("--out-json", type=str, default=None)
    args = parser.parse_args()

    ckw = json.loads(args.kwargs)
    jobs = [
        (g, args.base_seed + g * 1009, args.challenger, ckw, args.opponent, args.save_dir)
        for g in range(args.start_game, args.games)
    ]

    t0 = time.perf_counter()
    results: List[Dict[str, Any]] = []
    with ProcessPoolExecutor(max_workers=min(args.workers, len(jobs))) as ex:
        futs = [ex.submit(_run_one_game, j) for j in jobs]
        for fut in as_completed(futs):
            r = fut.result()
            results.append(r)
            tag = "WIN " if r["place"] == 1 else f"P{r['place']}  "
            print(
                f"[{len(results):3d}/{len(jobs)}] {tag} g={r['game_idx']:03d} T={r['turns']:2d} "
                f"cT={r['c_tier']} {r['c_comp']:<24} ({r['c_ms']:5.0f}ms/t) | wall={r['wall_s']:.1f}s",
                flush=True,
            )

    results.sort(key=lambda x: x["game_idx"])
    places = [r["place"] for r in results]
    n = len(places)
    avg = sum(places) / n
    win1 = 100.0 * sum(p == 1 for p in places) / n
    top2 = 100.0 * sum(p <= 2 for p in places) / n
    top4 = 100.0 * sum(p <= 4 for p in places) / n
    dist = {i: sum(p == i for p in places) for i in range(1, 9)}
    mean_c_ms = sum(r["c_ms"] for r in results) / n
    mean_o_ms = sum(r["opp_ms"] for r in results) / n

    print(f"\n=== 1v7 FFA Summary ({args.challenger} vs 7x {args.opponent}, n={n}, wall={time.perf_counter()-t0:.1f}s) ===")
    print(f"  1st Place: {win1:5.1f}% ({dist[1]}/{n}) | Top-2: {top2:5.1f}% | Top-4: {top4:5.1f}% | Avg Place: {avg:.2f}")
    print(f"  Placement Dist: {dist}")
    print(f"  Latency: {args.challenger}={mean_c_ms:.1f} ms/t | {args.opponent}={mean_o_ms:.1f} ms/t")

    losses = [r for r in results if r["place"] != 1][:5]
    if losses:
        print("\n--- Sample Losses (Challenger vs 1st-place SovereignBot) ---")
        for r in losses:
            print(f"  g={r['game_idx']:03d} P{r['place']} T={r['turns']} tribes={r['tribes']}")
            print(f"    Challenger (T{r['c_tier']}, {r['c_comp']}): {r['c_board']}")
            print(f"    Winner     (T{r['w_tier']}, seat={r['w_seat']}): {r['w_board']}")

    if args.out_json:
        with open(args.out_json, "w") as f:
            json.dump(results, f, indent=2)


if __name__ == "__main__":
    main()
