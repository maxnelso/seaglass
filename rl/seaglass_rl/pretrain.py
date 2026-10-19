"""Behavior cloning: pretrain a policy (and its value) on HeuristicBot games, to start PPO from.

    python -m seaglass_rl.pretrain --out runs/bc-solo.pt --games 400
    python -m seaglass_rl.pretrain --out runs/bc-duos.pt --duos --games 400
    python -m seaglass_rl.train_lobby --run-dir runs/solo --init runs/bc-solo.pt

An untrained policy places last against HeuristicBots almost every game. From there, a reward
that only comes at the end of the game is a weak signal. Cloning the bots first makes the PPO
learner start as good as the field, so its placement varies, and that variation is what PPO
learns from.

The data is every seat of `--games` all-HeuristicBot lobbies, encoded exactly as the learner sees
them (`play.py`, recording the bots). The policy is trained with cross-entropy on the bots'
actions. The value is regressed on the per-turn-discounted return (the placement, plus the
shaping if any).
"""

from __future__ import annotations

import argparse
import time

import numpy as np
import torch
import torch.nn as nn

from . import checkpoint
from .model import BattlegroundsMLP
from .play import LobbyConfig, GameSpec, Player
from .ppo import gae
from .train_lobby import play_options


def collect(config: LobbyConfig, games: int, workers: int, options, seed: int) -> dict[str, np.ndarray]:
    """The HeuristicBots' decisions in `games` lobbies, with their discounted returns."""
    specs = [GameSpec(seed=seed + g, controllers=["heuristic"] * config.units) for g in range(games)]
    player = Player(workers)
    try:
        results = player.play(config, specs, {}, options, record=("heuristic",), seed=seed)
    finally:
        player.close()
    trajs = [t for r in results for t in r.trajectories]
    returns = [
        gae(t["rewards"], np.zeros_like(t["rewards"]), t["turn_end"], True, options.gamma, 1.0)[1] for t in trajs
    ]
    data = {k: np.concatenate([t[k] for t in trajs]) for k in ("player_vec", "slot_card_ids", "slot_num_feats", "action_mask", "actions")}
    data["returns"] = np.concatenate(returns).astype(np.float32)
    return data


def train(
    model: BattlegroundsMLP,
    data: dict[str, np.ndarray],
    epochs: int,
    lr: float,
    minibatch: int,
    vf_coef: float = 0.5,
    holdout: float = 0.05,
    seed: int = 0,
) -> dict[str, float]:
    """Cross-entropy on the actions plus `vf_coef` × value MSE. Returns the held-out accuracy and
    value loss of the last epoch."""
    rng = np.random.default_rng(seed)
    n = len(data["actions"])
    perm = rng.permutation(n)
    n_val = max(1, int(n * holdout))
    val, tr = perm[:n_val], perm[n_val:]
    opt = torch.optim.Adam(model.parameters(), lr=lr)

    def tensors(idx: np.ndarray) -> tuple:
        return (
            torch.from_numpy(data["player_vec"][idx]),
            torch.from_numpy(data["slot_card_ids"][idx].astype(np.int64)),
            torch.from_numpy(data["slot_num_feats"][idx].astype(np.float32)),
            torch.from_numpy(data["action_mask"][idx]),
            torch.from_numpy(data["actions"][idx]),
            torch.from_numpy(data["returns"][idx]),
        )

    stats: dict[str, float] = {}
    for epoch in range(epochs):
        model.train()
        rng.shuffle(tr)
        t0 = time.time()
        for start in range(0, len(tr), minibatch):
            pv, ci, nf, mk, act, ret = tensors(tr[start : start + minibatch])
            logits, value = model(pv, ci, nf, mk)
            loss = nn.functional.cross_entropy(logits, act) + vf_coef * (value - ret).pow(2).mean()
            opt.zero_grad(set_to_none=True)
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            opt.step()
        model.eval()
        correct, vloss, ce = 0, 0.0, 0.0
        with torch.no_grad():
            for start in range(0, n_val, minibatch):
                pv, ci, nf, mk, act, ret = tensors(val[start : start + minibatch])
                logits, value = model(pv, ci, nf, mk)
                correct += int((logits.argmax(-1) == act).sum())
                ce += float(nn.functional.cross_entropy(logits, act, reduction="sum"))
                vloss += float((value - ret).pow(2).sum())
        stats = {"accuracy": correct / n_val, "cross_entropy": ce / n_val, "value_mse": vloss / n_val}
        print(
            f"epoch {epoch + 1}/{epochs} | held-out accuracy {stats['accuracy']:.1%} "
            f"ce {stats['cross_entropy']:.3f} value mse {stats['value_mse']:.3f} | {time.time() - t0:.0f}s",
            flush=True,
        )
    return stats


def main() -> None:
    from .train_lobby import parse_args as lobby_args

    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--out", required=True)
    p.add_argument("--duos", action="store_true")
    p.add_argument("--players", type=int, default=8)
    p.add_argument("--games", type=int, default=400)
    p.add_argument("--workers", type=int, default=12)
    p.add_argument("--epochs", type=int, default=4)
    p.add_argument("--lr", type=float, default=1e-3)
    p.add_argument("--minibatch", type=int, default=2048)
    p.add_argument("--hidden", type=int, default=256)
    p.add_argument("--embed", type=int, default=16)
    p.add_argument("--threads", type=int, default=12)
    p.add_argument("--seed", type=int, default=0)
    args = p.parse_args()
    torch.manual_seed(args.seed)
    torch.set_num_threads(args.threads)

    # The trainer's defaults for everything else (shaping, gamma, masks), so the values match.
    targs = lobby_args(["--run-dir", "-", *(["--duos"] if args.duos else []), "--players", str(args.players)])
    config = LobbyConfig(duos=args.duos, players=args.players, max_turns=targs.max_turns)
    t0 = time.time()
    data = collect(config, args.games, args.workers, play_options(targs), seed=10_000_000 + args.seed)
    print(f"{len(data['actions'])} decisions from {args.games} games in {time.time() - t0:.0f}s", flush=True)
    model = BattlegroundsMLP(hidden_dim=args.hidden, card_embed_dim=args.embed, seed=args.seed, kind_prior=True)
    stats = train(model, data, args.epochs, args.lr, args.minibatch, seed=args.seed)
    checkpoint.save(args.out, model, iteration=0, args=vars(args), extra={"bc": stats})
    print(f"saved {args.out}", flush=True)


if __name__ == "__main__":
    main()
