"""Behavior cloning: pretrain a policy (and its value) on rule-bot games, to start PPO from.

    python -m seaglass_rl.pretrain --out runs/bc-solo.pt --games 400
    python -m seaglass_rl.pretrain --out runs/bc-heur.pt --games 400 --teacher heuristic
    python -m seaglass_rl.pretrain --out runs/bc-duos.pt --duos --games 400
    python -m seaglass_rl.pretrain --out runs/bc-solo-tf.pt --model transformer --games 400
    python -m seaglass_rl.pretrain --out runs/bc-solo.pt --games 400 --device mps  # updates on the GPU
    python -m seaglass_rl.train_lobby --run-dir runs/solo --init runs/bc-solo.pt

An untrained policy places last against rule bots almost every game. From there, a reward
that only comes at the end of the game is a weak signal. Cloning the bots first makes the PPO
learner start as good as the field, so its placement varies, and that variation is what PPO
learns from.

The data is every seat of `--games` lobbies in which all seats are the `--teacher` bot (default
9kgamerbot), encoded exactly as the learner sees them (`play.py`, recording the bots). Games are
collected `--chunk` at a time, and only the arrays are kept. The policy is trained with
cross-entropy on the bots' actions. With a placement critic (`--value-head placement`, the
default), the value is trained with cross-entropy against the seat's final place, as in PPO;
with a scalar critic, it is regressed on the per-turn-discounted return (the placement, plus the
shaping if any). By default the critic is not trained here at all (`--value-epochs 0`): on
150 games of 9kgamerbot, its held-out cross-entropy on whole unseen games was 2.17 after one
epoch and 7.1 after twelve, against 2.08 for a uniform guess. It memorises each game's outcome,
and PPO then starts from confident wrong values. So its last layer is zeroed (a uniform guess)
and PPO trains it from its own games.

The move rules (`--no-reposition`, `--no-freeze`, `--no-positioning-phase`, `--prune`,
`--max-steps-per-turn`) apply to the recorded games and are saved in the checkpoint's arguments,
so give the PPO run that starts from it the same ones.
"""

from __future__ import annotations

import argparse
import dataclasses
import time

import numpy as np
import torch
import torch.nn as nn

from . import checkpoint, packing
from .device import add_device_arg, default_threads, default_workers, pick_device
from .play import LobbyConfig, GameSpec, Player
from .ppo import gae
from .train_lobby import play_options


def collect(
    config: LobbyConfig,
    games: int,
    workers: int,
    options,
    seed: int,
    teacher: str = "9kgamerbot",
    chunk: int = 40,
    pack: bool = True,
) -> dict[str, np.ndarray]:
    """The `teacher` bot's decisions in `games` lobbies (all seats), with their discounted returns
    and final places (0-based). Games are played `chunk` at a time; after each, only the needed
    arrays are kept. With `pack` (the default) the dataset stays packed (`packing.py`: `num_ptr`,
    `num_idx`, `num_val`, `mask_bits`, `packed`)."""
    options = dataclasses.replace(options, pack=pack)
    parts: list[dict[str, np.ndarray]] = []
    player = Player(workers)
    try:
        for start in range(0, games, max(1, chunk)):
            specs = [
                GameSpec(seed=seed + g, controllers=[teacher] * config.units)
                for g in range(start, min(games, start + max(1, chunk)))
            ]
            results = player.play(config, specs, {}, options, record=(teacher,), seed=seed + start)
            trajs = [t for r in results for t in r.trajectories]
            # Each decision's game (for a held-out split by game, not by decision).
            game_ids = [r.seed - seed for r in results for _ in r.trajectories]
            del results
            if not trajs:
                continue
            keys = ("player_vec", "slot_card_ids", "actions")
            keys += ("mask_bits",) if pack else ("slot_num_feats", "action_mask")
            part = {k: np.concatenate([t[k] for t in trajs]) for k in keys}
            if pack:
                part["num_ptr"], part["num_idx"], part["num_val"] = packing.concat_packed(
                    [(t["num_ptr"], t["num_idx"], t["num_val"]) for t in trajs]
                )
            part["returns"] = np.concatenate(
                [gae(t["rewards"], np.zeros_like(t["rewards"]), t["turn_end"], True, options.gamma, 1.0)[1] for t in trajs]
            ).astype(np.float32)
            part["places"] = np.concatenate(
                [np.full(len(t["actions"]), int(t["place"]) - 1 if int(t["place"]) > 0 else -1, dtype=np.int64) for t in trajs]
            )
            part["game"] = np.concatenate(
                [np.full(len(t["actions"]), gid, dtype=np.int32) for t, gid in zip(trajs, game_ids)]
            )
            parts.append(part)
    finally:
        player.close()
    data = {k: np.concatenate([p[k] for p in parts]) for k in parts[0] if not k.startswith("num_")}
    if pack:
        data["num_ptr"], data["num_idx"], data["num_val"] = packing.concat_packed(
            [(p["num_ptr"], p["num_idx"], p["num_val"]) for p in parts]
        )
        data["packed"] = np.asarray(True)
    return data


def train(
    model: torch.nn.Module,
    data: dict[str, np.ndarray],
    epochs: int,
    lr: float,
    minibatch: int,
    vf_coef: float = 0.5,
    holdout: float = 0.05,
    seed: int = 0,
    value_epochs: int | None = None,
) -> dict[str, float]:
    """Cross-entropy on the actions plus `vf_coef` × the value loss: cross-entropy against the final
    place with a placement critic, else MSE on the returns. Returns the held-out accuracy, action
    cross-entropy, value MSE (of the expected value against the returns) and, with a placement
    critic, value cross-entropy, for the last epoch.

    The held-out set is whole games (`data["game"]`), when known: every decision of a game shares
    its final place, so a split by decision lets the critic score well by recognising the game.
    The value loss is applied for the first `value_epochs` epochs only (all, if None): a critic
    trained longer memorises the games' outcomes, and PPO then starts from confident wrong values."""
    placement = model.config.get("value_places", 0) > 0
    rng = np.random.default_rng(seed)
    n = len(data["actions"])
    if "game" in data:
        games = np.unique(data["game"])
        held = rng.permutation(games)[: max(1, int(len(games) * holdout))]
        is_val = np.isin(data["game"], held)
        val, tr = np.nonzero(is_val)[0], np.nonzero(~is_val)[0]
    else:
        perm = rng.permutation(n)
        n_val = max(1, int(n * holdout))
        val, tr = perm[:n_val], perm[n_val:]
    n_val = len(val)
    device = next(model.parameters()).device
    opt = torch.optim.Adam(model.parameters(), lr=lr)

    packed = "num_ptr" in data

    def tensors(idx: np.ndarray) -> tuple:
        if packed:
            feats = packing.unpack_rows(data["num_ptr"], data["num_idx"], data["num_val"], idx)
            mask = packing.unpack_mask(data["mask_bits"], idx)
        else:
            feats = data["slot_num_feats"][idx].astype(np.float32)  # stored as float16
            mask = data["action_mask"][idx]
        return (
            torch.from_numpy(data["player_vec"][idx]),
            torch.from_numpy(data["slot_card_ids"][idx].astype(np.int64)),
            torch.from_numpy(feats),
            torch.from_numpy(mask),
            torch.from_numpy(data["actions"][idx]),
            torch.from_numpy(data["returns"][idx]),
            torch.from_numpy(data["places"][idx]),
        )

    stats: dict[str, float] = {}
    for epoch in range(epochs):
        model.train()
        rng.shuffle(tr)
        t0 = time.time()
        for start in range(0, len(tr), minibatch):
            pv, ci, nf, mk, act, ret, plc = (t.to(device) for t in tensors(tr[start : start + minibatch]))
            logits, value, value_logits = model(pv, ci, nf, mk, return_value_logits=True)
            if placement:
                vloss_t = nn.functional.cross_entropy(value_logits, plc, ignore_index=-1)
            else:
                vloss_t = (value - ret).pow(2).mean()
            vf = vf_coef if value_epochs is None or epoch < value_epochs else 0.0
            loss = nn.functional.cross_entropy(logits, act) + vf * vloss_t
            opt.zero_grad(set_to_none=True)
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            opt.step()
        model.eval()
        correct, vloss, ce, vce = 0, 0.0, 0.0, 0.0
        with torch.no_grad():
            for start in range(0, n_val, minibatch):
                pv, ci, nf, mk, act, ret, plc = (t.to(device) for t in tensors(val[start : start + minibatch]))
                logits, value, value_logits = model(pv, ci, nf, mk, return_value_logits=True)
                if placement:
                    vce += float(nn.functional.cross_entropy(value_logits, plc, ignore_index=-1, reduction="sum"))
                correct += int((logits.argmax(-1) == act).sum())
                ce += float(nn.functional.cross_entropy(logits, act, reduction="sum"))
                vloss += float((value - ret).pow(2).sum())
        stats = {"accuracy": correct / n_val, "cross_entropy": ce / n_val, "value_mse": vloss / n_val}
        if placement:
            stats["value_ce"] = vce / n_val
            value_text = f"value ce {stats['value_ce']:.3f} (mse {stats['value_mse']:.3f})"
        else:
            value_text = f"value mse {stats['value_mse']:.3f}"
        print(
            f"epoch {epoch + 1}/{epochs} | held-out accuracy {stats['accuracy']:.1%} "
            f"ce {stats['cross_entropy']:.3f} {value_text} | {time.time() - t0:.0f}s",
            flush=True,
        )
    if value_epochs == 0:
        reset_critic_head(model)
    return stats


def reset_critic_head(model: torch.nn.Module) -> None:
    """Zero the critic's last layer, so an untrained critic predicts a uniform place (value 0)
    instead of whatever the policy-trained features happen to give."""
    critic = model.critic
    last = critic if isinstance(critic, nn.Linear) else [m for m in critic.modules() if isinstance(m, nn.Linear)][-1]
    with torch.no_grad():
        last.weight.zero_()
        last.bias.zero_()


def main() -> None:
    from .train_lobby import add_model_args, build_model, parse_args as lobby_args

    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--out", required=True)
    p.add_argument("--duos", action="store_true")
    p.add_argument("--players", type=int, default=8)
    p.add_argument("--games", type=int, default=400)
    p.add_argument("--chunk", type=int, default=40, help="games played per batch while collecting (bounds memory)")
    p.add_argument(
        "--teacher",
        choices=("heuristic", "9kgamerbot", "9kgamerbot-tempo"),
        default="9kgamerbot",
        help="the bot recorded in every seat",
    )
    # The move rules, as in `train_lobby.py`: the PPO run that starts from this checkpoint should use
    # the same ones (they are saved with it, and `ladder.py` rates it under them).
    p.add_argument(
        "--max-steps-per-turn", type=int, default=None, help="as in train_lobby (default: none, the turn timer only)"
    )
    p.add_argument("--no-reposition", action="store_true")
    p.add_argument("--auto-position", action="store_true", help="recorded with the checkpoint; the bots do not use it")
    p.add_argument("--no-positioning-phase", action="store_true")
    p.add_argument("--no-freeze", action="store_true")
    p.add_argument("--prune", action="store_true", help="mask the sells and refreshes `prune_dominated` rules out")
    p.add_argument("--workers", type=int, default=default_workers(), help="default: $SEAGLASS_WORKERS, else half the cores")
    p.add_argument("--epochs", type=int, default=4)
    p.add_argument(
        "--value-epochs",
        type=int,
        default=0,
        help="train the critic for the first N epochs only (default 0: not at all, and its last layer is zeroed; it memorises the games)",
    )
    p.add_argument("--lr", type=float, default=1e-3)
    p.add_argument("--minibatch", type=int, default=2048)
    add_model_args(p)
    p.add_argument("--threads", type=int, default=default_threads(), help="default: $SEAGLASS_THREADS, else half the cores")
    add_device_arg(p)
    p.add_argument("--no-pack", action="store_true", help="keep the dataset dense (about 20x the memory)")
    p.add_argument("--seed", type=int, default=0)
    args = p.parse_args()
    device = pick_device(args.device)
    args.device = str(device)  # recorded with the checkpoint's args
    torch.manual_seed(args.seed)
    torch.set_num_threads(args.threads)

    # The trainer's defaults for everything else (shaping, gamma, masks), so the values match.
    rules = ["--max-steps-per-turn", str(args.max_steps_per_turn)] if args.max_steps_per_turn is not None else []
    rules += [f for f, on in (
        ("--no-reposition", args.no_reposition),
        ("--auto-position", args.auto_position),
        ("--no-positioning-phase", args.no_positioning_phase),
        ("--no-freeze", args.no_freeze),
        ("--prune", args.prune),
    ) if on]
    targs = lobby_args(["--run-dir", "-", *(["--duos"] if args.duos else []), "--players", str(args.players), *rules])
    config = LobbyConfig(duos=args.duos, players=args.players, max_turns=targs.max_turns)
    t0 = time.time()
    data = collect(
        config, args.games, args.workers, play_options(targs), seed=10_000_000 + args.seed, teacher=args.teacher, chunk=args.chunk,
        pack=not args.no_pack,
    )
    print(f"{len(data['actions'])} decisions from {args.games} games in {time.time() - t0:.0f}s", flush=True)
    model = build_model(args, kind_prior=True, units=config.units).to(device)
    stats = train(model, data, args.epochs, args.lr, args.minibatch, seed=args.seed, value_epochs=args.value_epochs)
    checkpoint.save(args.out, model, iteration=0, args=vars(args), extra={"bc": stats})
    print(f"saved {args.out}", flush=True)


if __name__ == "__main__":
    main()
