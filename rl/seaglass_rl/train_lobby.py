"""Train a policy with PPO in full Battlegrounds lobbies: 8 Solo players, or Duos (4 teams of 2).

    python -m seaglass_rl.train_lobby --run-dir runs/solo --iterations 500
    python -m seaglass_rl.train_lobby --run-dir runs/duos --duos --iterations 500
    python -m seaglass_rl.train_lobby --run-dir runs/solo --resume runs/solo/latest.pt
    python -m seaglass_rl.train_lobby --run-dir runs/solo-tf --model transformer  # `transformer.py`

Each iteration:
1. **Play** `--games` lobbies across `--workers` processes. In each one, the learner controls
   `--learner-units` units (Solo players, or whole Duos teams: both teammates share the policy).
   The opponent pool fills the rest: HeuristicBots and frozen snapshots of the learner, taken
   every `--snapshot-every` iterations, with the latest `--pool-size` kept.
2. **Update**: PPO on the learner's decisions (`ppo.py`). The reward is the unit's placement
   (`play.placement_reward`), plus optional potential-based shaping (`--shaping`). Discounting
   happens per turn.
3. **Every `--eval-every` iterations**: evaluate the learner's average place, alone against
   HeuristicBots (`evaluate.py`), and save `latest.pt`.

The run directory holds `args.json`, `log.jsonl` (one line per iteration), `latest.pt` and
`snapshots/iter_XXXXX.pt`.
"""

from __future__ import annotations

import argparse
import json
import random
import time
from pathlib import Path
from typing import Any

import numpy as np
import torch

from . import checkpoint
from .behavior import behavior_stats, format_behavior
from .evaluate import evaluate, format_result
from .model import BattlegroundsMLP
from .play import LobbyConfig, LEARNER, GameSpec, Player, PlayOptions
from .ppo import build_batch, ppo_update
from .transformer import EntityTransformer


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--run-dir", required=True)
    p.add_argument("--resume", default=None, help="continue from a checkpoint (weights, optimizer, iteration)")
    p.add_argument("--init", default=None, help="start from a checkpoint's weights")
    # The game
    p.add_argument("--duos", action="store_true")
    p.add_argument("--players", type=int, default=8)
    p.add_argument("--max-turns", type=int, default=30)
    p.add_argument("--no-heroes", action="store_true")
    p.add_argument("--max-steps-per-turn", type=int, default=30)
    p.add_argument("--no-reposition", action="store_true")
    p.add_argument(
        "--no-positioning-phase",
        action="store_true",
        help="let the model reposition and freeze at any time, not only in the end-of-turn step",
    )
    p.add_argument("--no-damage-caps", action="store_true", help="apply combat damage in full")
    p.add_argument("--tribes", type=int, default=5, help="minion types per lobby (0: every type)")
    p.add_argument("--no-freeze", action="store_true")
    p.add_argument("--prune", action="store_true", help="mask the sells and refreshes `prune_dominated` rules out")
    # Collection
    p.add_argument("--iterations", type=int, default=500)
    p.add_argument("--games", type=int, default=64, help="lobbies per iteration")
    p.add_argument("--workers", type=int, default=12)
    p.add_argument("--learner-units", type=int, default=None, help="default: half the units")
    p.add_argument("--p-heuristic", type=float, default=None, help="chance an opponent is a HeuristicBot (default 0.3; 0 with --pure)")
    p.add_argument(
        "--pure",
        action="store_true",
        help="pure self-play: no HeuristicBots in training; the learner fills every seat until the first "
        "snapshot, then plays against its snapshots (evaluation still uses HeuristicBots)",
    )
    p.add_argument("--snapshot-every", type=int, default=10)
    p.add_argument("--pool-size", type=int, default=10)
    # PPO
    p.add_argument("--lr", type=float, default=3e-4)
    p.add_argument("--anneal-lr", action="store_true")
    p.add_argument("--epochs", type=int, default=4)
    p.add_argument("--minibatch", type=int, default=4096)
    p.add_argument("--gamma", type=float, default=0.97, help="discount per turn")
    p.add_argument("--lam", type=float, default=0.95)
    p.add_argument("--clip", type=float, default=0.2)
    p.add_argument("--ent-coef", type=float, default=0.003)
    p.add_argument("--vf-coef", type=float, default=0.5)
    p.add_argument("--target-kl", type=float, default=0.03)
    p.add_argument("--shaping", type=float, default=0.3, help="coefficient of the board-power potential (0: off)")
    p.add_argument("--health-shaping", type=float, default=0.0, help="coefficient of the Health potential (0: off)")
    # Model
    add_model_args(p)
    p.add_argument("--no-kind-prior", action="store_true", help="spread the untrained policy over actions, not kinds")
    p.add_argument("--threads", type=int, default=8, help="torch threads for the update")
    # Evaluation
    p.add_argument("--eval-every", type=int, default=20)
    p.add_argument("--eval-games", type=int, default=96)
    p.add_argument("--replays", action="store_true", help="save a few eval games as replays in RUN_DIR/replays/iter_XXXXX")
    p.add_argument("--seed", type=int, default=0)
    return p.parse_args(argv)


def add_model_args(p: argparse.ArgumentParser) -> None:
    """The model's architecture flags (shared with `pretrain.py`; `build_model`)."""
    p.add_argument("--model", choices=("mlp", "transformer"), default="mlp")
    p.add_argument("--hidden", type=int, default=256, help="MLP: hidden width")
    p.add_argument("--embed", type=int, default=16, help="the card embedding's size")
    p.add_argument(
        "--no-static-features", action="store_true", help="leave out the static card features (`card_features.py`)"
    )
    p.add_argument("--d-model", type=int, default=64, help="transformer: token width")
    p.add_argument("--layers", type=int, default=3, help="transformer: encoder layers")
    p.add_argument("--heads", type=int, default=4, help="transformer: attention heads")
    p.add_argument("--ff", type=int, default=128, help="transformer: feed-forward width")


def build_model(args: argparse.Namespace, kind_prior: bool) -> torch.nn.Module:
    """A fresh model of the architecture `args` name (`add_model_args`)."""
    if args.model == "transformer":
        return EntityTransformer(
            card_embed_dim=args.embed,
            d_model=args.d_model,
            layers=args.layers,
            heads=args.heads,
            ff=args.ff,
            head_dim=args.d_model,
            seed=args.seed,
            kind_prior=kind_prior,
            static_features=not args.no_static_features,
        )
    return BattlegroundsMLP(
        hidden_dim=args.hidden,
        card_embed_dim=args.embed,
        seed=args.seed,
        kind_prior=kind_prior,
        static_features=not args.no_static_features,
    )


def lobby_config(args: argparse.Namespace) -> LobbyConfig:
    return LobbyConfig(
        duos=args.duos,
        players=args.players,
        max_turns=args.max_turns,
        heroes=not args.no_heroes,
        damage_caps=not args.no_damage_caps,
        tribes=args.tribes,
    )


def play_options(args: argparse.Namespace, deterministic: bool = False) -> PlayOptions:
    return PlayOptions(
        max_steps_per_turn=args.max_steps_per_turn,
        positioning_phase=not args.no_positioning_phase,
        allow_reposition=not args.no_reposition,
        allow_freeze=not args.no_freeze,
        prune_dominated=args.prune,
        deterministic=deterministic,
        gamma=args.gamma,
        shaping=args.shaping,
        health_shaping=args.health_shaping,
    )


def make_specs(
    config: LobbyConfig, games: int, learner_units: int, pool: list[str], p_heuristic: float, rng: random.Random, seed: int
) -> list[GameSpec]:
    """The iteration's games: the learner in `learner_units` random units, the pool in the rest. With
    no snapshot yet and no HeuristicBots (`p_heuristic` 0), the learner fills every unit."""
    specs = []
    for g in range(games):
        controllers = []
        everyone = not pool and p_heuristic <= 0
        mine = set(range(config.units)) if everyone else set(rng.sample(range(config.units), learner_units))
        for u in range(config.units):
            if u in mine:
                controllers.append(LEARNER)
            elif not pool or rng.random() < p_heuristic:
                controllers.append("heuristic")
            else:
                controllers.append(rng.choice(pool))
        specs.append(GameSpec(seed=seed + g, controllers=controllers))
    return specs


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)
    run = Path(args.run_dir)
    (run / "snapshots").mkdir(parents=True, exist_ok=True)
    torch.manual_seed(args.seed)
    torch.set_num_threads(args.threads)
    rng = random.Random(args.seed)
    config = lobby_config(args)
    learner_units = args.learner_units or max(1, config.units // 2)
    if args.p_heuristic is None:
        args.p_heuristic = 0.0 if args.pure else 0.3
    if args.pure and (args.init or args.p_heuristic > 0):
        raise SystemExit("--pure learns from scratch: no --init and no HeuristicBots in training")

    start = 0
    if args.resume:
        model, ckpt = checkpoint.load(args.resume)
        start = int(ckpt["iteration"])
    elif args.init:
        model, _ = checkpoint.load(args.init)
    else:
        model = build_model(args, kind_prior=not args.no_kind_prior)
    model.eval()
    optimizer = torch.optim.Adam(model.parameters(), lr=args.lr, eps=1e-5)
    if args.resume and "optimizer_state" in ckpt:
        optimizer.load_state_dict(ckpt["optimizer_state"])
    (run / "args.json").write_text(json.dumps(vars(args), indent=2))
    pool = sorted(str(p) for p in (run / "snapshots").glob("iter_*.pt"))[-args.pool_size :]

    n_params = sum(p.numel() for p in model.parameters())
    print(
        f"[{'Duos' if config.duos else 'Solo'}] {config.players} players, learner in {learner_units}/"
        f"{config.units} units{' (pure self-play: every unit until the first snapshot)' if args.pure else ''}, "
        f"{args.games} games/iter, {n_params / 1e6:.2f}M params",
        flush=True,
    )
    player = Player(args.workers)
    log = open(run / "log.jsonl", "a", encoding="utf-8")
    # What the HeuristicBots do, to read the learner's behavior against.
    reference = player.play(
        config,
        [GameSpec(seed=7_000_000 + g, controllers=["heuristic"] * config.units) for g in range(16)],
        {},
        play_options(args),
        record=("heuristic",),
    )
    print("  HeuristicBot behavior (reference):\n" + format_behavior(
        behavior_stats([t for r in reference for t in r.trajectories])), flush=True)
    t_start = time.time()
    try:
        for it in range(start + 1, start + args.iterations + 1):
            t0 = time.time()
            if args.anneal_lr:
                frac = 1.0 - (it - start - 1) / args.iterations
                for g in optimizer.param_groups:
                    g["lr"] = args.lr * frac
            specs = make_specs(
                config, args.games, learner_units, pool, args.p_heuristic, rng, seed=args.seed * 10_000_019 + it * 1000
            )
            source = (dict(model.config), {k: v.detach().clone() for k, v in model.state_dict().items()})
            sources: dict[str, Any] = {LEARNER: source}
            sources.update({p: p for p in pool})
            results = player.play(config, specs, sources, play_options(args), seed=it)
            t_play = time.time() - t0
            trajs = [t for r in results for t in r.trajectories]
            batch = build_batch(trajs, args.gamma, args.lam)
            metrics = ppo_update(
                model,
                optimizer,
                batch,
                epochs=args.epochs,
                minibatch_size=args.minibatch,
                clip_eps=args.clip,
                vf_coef=args.vf_coef,
                ent_coef=args.ent_coef,
                target_kl=args.target_kl,
            )
            places = [r.placements[u] for r in results for u in range(config.units) if r.controllers[u] == LEARNER]
            row: dict[str, Any] = {
                "iter": it,
                "steps": int(batch["actions"].shape[0]),
                "learner_place": float(np.mean(places)),
                "turns": float(np.mean([r.turns for r in results])),
                "play_s": round(t_play, 1),
                "update_s": round(time.time() - t0 - t_play, 1),
                **{k: round(v, 4) for k, v in metrics.items()},
            }
            if it % args.snapshot_every == 0:
                snap = run / "snapshots" / f"iter_{it:05d}.pt"
                checkpoint.save(snap, model, iteration=it, args=vars(args))
                pool = (pool + [str(snap)])[-args.pool_size :]
            if it % args.eval_every == 0 or it == start + args.iterations:
                checkpoint.save(run / "latest.pt", model, optimizer, iteration=it, args=vars(args))
                res = evaluate(
                    (dict(model.config), model.state_dict()),
                    ["heuristic"],
                    config,
                    games=args.eval_games,
                    agent_units=1,
                    seed=999_000,
                    player=player,
                    options=play_options(args),
                    replay_dir=str(run / "replays" / f"iter_{it:05d}") if args.replays else None,
                )
                row["eval_vs_heuristic"] = res
                row["behavior"] = behavior_stats(trajs)
                print("  " + format_result(f"eval iter {it}", res), flush=True)
                print(format_behavior(row["behavior"]), flush=True)
            print(
                f"iter {it:5d} | place {row['learner_place']:.2f} | steps {row['steps']:6d} | "
                f"turns {row['turns']:.1f} | pi {row['policy_loss']:+.3f} v {row['value_loss']:.3f} "
                f"ent {row['entropy']:.2f} kl {row['kl']:.3f} ev {row['explained_var']:.2f} | "
                f"play {row['play_s']}s upd {row['update_s']}s | {time.time() - t_start:.0f}s",
                flush=True,
            )
            log.write(json.dumps(row) + "\n")
            log.flush()
    finally:
        log.close()
        player.close()


if __name__ == "__main__":
    main()
