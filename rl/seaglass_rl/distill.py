"""Distilling the in-turn search into the network (expert iteration, offline).

    python -m seaglass_rl.distill generate --model runs/solo/latest.pt --lobbies 2000 --out runs/distill/data --workers 6
    python -m seaglass_rl.distill train --init runs/solo/latest.pt --data runs/distill/data --out runs/distill/student.pt
    # the gate: the distilled network against the one it started from, both without search
    python -m seaglass_rl.ladder versus runs/distill/student.pt runs/solo/latest.pt --games 200

`generate` plays Solo lobbies in which `--search-seats` seats are the model with search (`searcher`,
Gumbel noise at the root, as in Gumbel MuZero) and the rest the same checkpoint without search
(`rival`), under the checkpoint's own move rules. Only the searchers are recorded: for each decision,
the packed observation, the action played, the search's improved policy (`search_policy`, zero where
the decision was not searched: `searched` marks the others) and the seat's final place. One `.npz`
shard per `--chunk` lobbies, plus `meta.json`.

`train` starts from the checkpoint and fits three terms, on minibatches densified from the packed data:
- the policy: cross-entropy of the network's masked log-softmax against `search_policy`, on the
  searched rows;
- the value: the placement critic's cross-entropy against the final place, on every row (a scalar
  critic is refused);
- the anchor: `anchor_coef` times KL(pi_init || pi_theta) on every row, with pi_init from a frozen copy
  of the starting network, so fitting the targets doesn't drift far from a policy known to be strong.
Whole games are held out. Every epoch reports, on them, the policy cross-entropy, `agree` (the share of
searched rows where the network's argmax is the search's argmax, next to the starting network's), the
value cross-entropy and the KL to the start. The output keeps the checkpoint's `args`, so
`ladder.checkpoint_options` rates it under the same move rules, and adds a `distill` entry to `extra`.
"""

from __future__ import annotations

import argparse
import copy
import dataclasses
import json
import random
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import numpy as np
import torch
import torch.nn.functional as F

from . import checkpoint, packing
from .actions import ACTION_SPACE_SIZE
from .device import add_device_arg, default_threads, default_workers, pick_device
from .ladder import _commit_info, _engine_fingerprint, checkpoint_options
from .play import GameSpec, LobbyConfig, Player
from .search import SearchSpec

SEARCHER = "searcher"
RIVAL = "rival"
_PACKED_KEYS = ("player_vec", "slot_card_ids", "mask_bits", "actions")


# -- Generating -----------------------------------------------------------------------------------


def _shard_arrays(results: list[Any], first_game: int, seed: int) -> dict[str, np.ndarray] | None:
    """The searchers' decisions in `results` as one shard (`first_game` is unused: the lobby's index is
    `r.seed - seed`)."""
    parts: list[dict[str, np.ndarray]] = []
    for r in sorted(results, key=lambda r: r.seed):
        for t in r.trajectories:
            n = len(t["actions"])
            if n == 0:
                continue
            place = int(t["place"])
            part = {k: t[k] for k in _PACKED_KEYS}
            part["search_policy"] = t["search_policy"].astype(np.float16)
            part["places"] = np.full(n, place - 1 if place > 0 else -1, dtype=np.int64)
            part["game"] = np.full(n, r.seed - seed, dtype=np.int32)
            part["_packed"] = (t["num_ptr"], t["num_idx"], t["num_val"])  # type: ignore[assignment]
            parts.append(part)
    if not parts:
        return None
    out = {k: np.concatenate([p[k] for p in parts]) for k in parts[0] if k != "_packed"}
    out["num_ptr"], out["num_idx"], out["num_val"] = packing.concat_packed([p["_packed"] for p in parts])  # type: ignore[misc]
    out["searched"] = out["search_policy"].astype(np.float32).sum(axis=1) > 0
    return out


def generate(
    model_path: str,
    out_dir: str | Path,
    lobbies: int,
    search_seats: int = 2,
    workers: int = 0,
    seed: int = 9_000_000,
    chunk: int = 50,
    spec: SearchSpec | None = None,
    config: LobbyConfig | None = None,
    player: Player | None = None,
) -> dict[str, Any]:
    """Play `lobbies` Solo lobbies and write the searchers' decisions to `out_dir` (see the module
    docstring). Returns the meta (also saved as `meta.json`), with the decision counts."""
    config = config or LobbyConfig()
    if config.duos or config.players != 8:
        raise ValueError("distill plays 8-player Solo lobbies")
    if not 1 <= search_seats <= config.units:
        raise ValueError(f"search_seats must be in 1..{config.units}")
    spec = spec or SearchSpec()
    spec = dataclasses.replace(spec, gumbel_noise=True)
    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)
    options = dataclasses.replace(
        checkpoint_options(model_path), search=spec, search_controllers=(SEARCHER,), pack=True
    )
    rng = random.Random(seed)
    sources = {SEARCHER: model_path, RIVAL: model_path}
    own = player is None
    player = player or Player(workers)
    t0 = time.time()
    decisions = searched = 0
    shards = []
    try:
        for ci, start in enumerate(range(0, lobbies, max(1, chunk))):
            specs = []
            for g in range(start, min(lobbies, start + max(1, chunk))):
                seats = [SEARCHER] * search_seats + [RIVAL] * (config.units - search_seats)
                rng.shuffle(seats)
                specs.append(GameSpec(seed=seed + g, controllers=seats))
            results = player.play(config, specs, sources, options, record=(SEARCHER,), seed=seed + start)
            shard = _shard_arrays(results, start, seed)
            del results
            if shard is None:
                continue
            name = f"shard_{ci:05d}.npz"
            np.savez_compressed(out / name, **shard)
            shards.append(name)
            decisions += len(shard["actions"])
            searched += int(shard["searched"].sum())
            print(
                f"chunk {ci + 1}: lobbies {start}-{start + len(specs) - 1} | {len(shard['actions'])} decisions, "
                f"{int(shard['searched'].sum())} searched | {time.time() - t0:.0f}s",
                flush=True,
            )
    finally:
        if own:
            player.close()
    commit, dirty = _commit_info()
    meta = {
        "model": str(model_path),
        "search": dataclasses.asdict(spec),
        "search_seats": search_seats,
        "lobbies": lobbies,
        "seed": seed,
        "chunk": chunk,
        "shards": shards,
        "decisions": decisions,
        "searched": searched,
        "commit": commit,
        "dirty": dirty,
        "engine": _engine_fingerprint(),
        "created": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "seconds": round(time.time() - t0, 1),
    }
    (out / "meta.json").write_text(json.dumps(meta, indent=2))
    print(f"{decisions} decisions ({searched} searched) from {lobbies} lobbies in {time.time() - t0:.0f}s", flush=True)
    return meta


# -- Training -------------------------------------------------------------------------------------


def load_data(data_dir: str | Path) -> dict[str, np.ndarray]:
    """All the shards of `data_dir`, concatenated and still packed."""
    files = sorted(Path(data_dir).glob("shard_*.npz"))
    if not files:
        raise FileNotFoundError(f"no shards in {data_dir}")
    parts = []
    for f in files:
        with np.load(f) as z:
            parts.append({k: z[k] for k in z.files})
    data = {k: np.concatenate([p[k] for p in parts]) for k in parts[0] if not k.startswith("num_")}
    data["num_ptr"], data["num_idx"], data["num_val"] = packing.concat_packed(
        [(p["num_ptr"], p["num_idx"], p["num_val"]) for p in parts]
    )
    return data


def batch_tensors(data: dict[str, np.ndarray], idx: np.ndarray, device: torch.device) -> dict[str, torch.Tensor]:
    """The rows `idx` of the packed `data`, dense, on `device`."""
    feats = packing.unpack_rows(data["num_ptr"], data["num_idx"], data["num_val"], idx)
    mask = packing.unpack_mask(data["mask_bits"], idx)
    t = {
        "player_vec": torch.from_numpy(data["player_vec"][idx]),
        "card_ids": torch.from_numpy(data["slot_card_ids"][idx].astype(np.int64)),
        "num_feats": torch.from_numpy(feats),
        "mask": torch.from_numpy(mask),
        "target": torch.from_numpy(data["search_policy"][idx].astype(np.float32)),
        "searched": torch.from_numpy(data["searched"][idx]),
        "places": torch.from_numpy(data["places"][idx]),
    }
    return {k: v.to(device) for k, v in t.items()}


def _forward(model: torch.nn.Module, b: dict[str, torch.Tensor]) -> tuple[torch.Tensor, torch.Tensor]:
    logits, _, value_logits = model(b["player_vec"], b["card_ids"], b["num_feats"], b["mask"], return_value_logits=True)
    return logits, value_logits


def loss_terms(
    model: torch.nn.Module, init_logits: torch.Tensor, b: dict[str, torch.Tensor]
) -> dict[str, torch.Tensor]:
    """The three loss terms and the diagnostics on a batch `b` (`batch_tensors`), given the frozen
    starting network's logits on it: `policy` (cross-entropy against the search's policy, searched rows
    only; 0 with none), `value` (place cross-entropy, all rows), `kl` (KL(pi_init || pi_theta), all
    rows), and `agree` (the searched rows where the argmax matches; as a count in `agree_n`)."""
    logits, value_logits = _forward(model, b)
    logp = F.log_softmax(logits, dim=-1)
    s = b["searched"]
    n_s = s.sum()
    if int(n_s) > 0:
        ce = -(b["target"][s] * logp[s]).sum(-1)
        policy = ce.mean()
        agree = (logits[s].argmax(-1) == b["target"][s].argmax(-1)).sum()
    else:
        policy = logits.sum() * 0.0
        agree = torch.zeros((), dtype=torch.long, device=logits.device)
    value = F.cross_entropy(value_logits, b["places"], ignore_index=-1)
    p_init = F.softmax(init_logits, dim=-1)
    logp_init = F.log_softmax(init_logits, dim=-1)
    kl = torch.where(p_init > 0, p_init * (logp_init - logp), torch.zeros_like(logp)).sum(-1).mean()
    return {"policy": policy, "value": value, "kl": kl, "agree": agree, "searched": n_s}


def split_games(game: np.ndarray, holdout: float, seed: int) -> tuple[np.ndarray, np.ndarray]:
    """`(train rows, held-out rows)`: whole games held out (at least one, if there are two or more)."""
    rng = np.random.default_rng(seed)
    games = np.unique(game)
    n_held = min(len(games) - 1, max(1, int(len(games) * holdout))) if len(games) > 1 else 0
    held = rng.permutation(games)[:n_held]
    is_val = np.isin(game, held)
    return np.nonzero(~is_val)[0], np.nonzero(is_val)[0]


def _check_critic(model: torch.nn.Module) -> None:
    if model.config.get("value_places", 0) <= 0:
        raise ValueError("distill trains a placement critic: the init checkpoint has a scalar critic (value_head scalar)")


@torch.no_grad()
def evaluate(
    model: torch.nn.Module,
    init: torch.nn.Module,
    data: dict[str, np.ndarray],
    rows: np.ndarray,
    minibatch: int,
    device: torch.device,
) -> dict[str, float]:
    """Held-out metrics over `rows`: policy cross-entropy and `agree` (searched rows), their value
    cross-entropy and KL to the init (all rows), and the init's own `init_agree`."""
    was = model.training
    model.eval()
    tot = {"policy": 0.0, "value": 0.0, "kl": 0.0, "agree": 0, "init_agree": 0, "searched": 0}
    for start in range(0, len(rows), minibatch):
        b = batch_tensors(data, rows[start : start + minibatch], device)
        init_logits, _ = _forward(init, b)
        t = loss_terms(model, init_logits, b)
        n, ns = len(b["places"]), int(t["searched"])
        tot["policy"] += float(t["policy"]) * ns
        tot["value"] += float(t["value"]) * n
        tot["kl"] += float(t["kl"]) * n
        tot["agree"] += int(t["agree"])
        s = b["searched"]
        if ns:
            tot["init_agree"] += int((init_logits[s].argmax(-1) == b["target"][s].argmax(-1)).sum())
        tot["searched"] += ns
    model.train(was)
    n, ns = max(1, len(rows)), max(1, tot["searched"])
    return {
        "policy_ce": tot["policy"] / ns,
        "agree": tot["agree"] / ns,
        "init_agree": tot["init_agree"] / ns,
        "value_ce": tot["value"] / n,
        "kl_to_init": tot["kl"] / n,
        "heldout_rows": len(rows),
        "heldout_searched": tot["searched"],
    }


def train(
    model: torch.nn.Module,
    data: dict[str, np.ndarray],
    epochs: int = 2,
    lr: float = 1e-5,
    minibatch: int = 2048,
    vf_coef: float = 0.5,
    anchor_coef: float = 0.1,
    holdout: float = 0.05,
    policy_coef: float = 1.0,
    seed: int = 0,
    init: torch.nn.Module | None = None,
) -> list[dict[str, float]]:
    """Fit `model` to the search's policy and the final places, anchored to `init` (default: a frozen
    copy of `model` as it comes). Returns the held-out metrics: one entry before training (epoch 0)
    and one per epoch."""
    _check_critic(model)
    device = next(model.parameters()).device
    if init is None:
        init = copy.deepcopy(model)
    init.eval().requires_grad_(False)
    tr, val = split_games(data["game"], holdout, seed)
    rng = np.random.default_rng(seed)
    opt = torch.optim.Adam(model.parameters(), lr=lr)
    history = []

    def report(epoch: int, secs: float) -> None:
        m = evaluate(model, init, data, val, minibatch, device) if len(val) else {}
        m["epoch"] = epoch
        history.append(m)
        if m.get("heldout_rows"):
            print(
                f"epoch {epoch}/{epochs} | held-out policy ce {m['policy_ce']:.3f} | agree {m['agree']:.1%} "
                f"(init {m['init_agree']:.1%}) | value ce {m['value_ce']:.3f} | kl to init {m['kl_to_init']:.4f} | {secs:.0f}s",
                flush=True,
            )
        else:
            print(f"epoch {epoch}/{epochs} | no held-out games | {secs:.0f}s", flush=True)

    report(0, 0.0)
    for epoch in range(1, epochs + 1):
        model.train()
        t0 = time.time()
        order = rng.permutation(tr)
        for start in range(0, len(order), minibatch):
            b = batch_tensors(data, order[start : start + minibatch], device)
            with torch.no_grad():
                init_logits, _ = _forward(init, b)
            t = loss_terms(model, init_logits, b)
            loss = policy_coef * t["policy"] + vf_coef * t["value"] + anchor_coef * t["kl"]
            opt.zero_grad(set_to_none=True)
            loss.backward()
            torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            opt.step()
        model.eval()
        report(epoch, time.time() - t0)
    return history


def run_train(
    init_path: str,
    data_dir: str,
    out: str,
    epochs: int = 2,
    lr: float = 1e-5,
    minibatch: int = 2048,
    vf_coef: float = 0.5,
    anchor_coef: float = 0.1,
    holdout: float = 0.05,
    device: str | None = None,
    seed: int = 0,
) -> list[dict[str, float]]:
    model, ckpt = checkpoint.load(init_path)
    _check_critic(model)
    dev = pick_device(device)
    model.to(dev)
    data = load_data(data_dir)
    n_s = int(data["searched"].sum())
    print(f"{len(data['actions'])} decisions ({n_s} searched), {len(np.unique(data['game']))} games", flush=True)
    torch.manual_seed(seed)
    history = train(model, data, epochs, lr, minibatch, vf_coef, anchor_coef, holdout, seed=seed)
    meta_path = Path(data_dir) / "meta.json"
    distill = {
        "init": str(init_path),
        "data": str(data_dir),
        "data_meta": json.loads(meta_path.read_text()) if meta_path.exists() else None,
        "epochs": epochs,
        "lr": lr,
        "minibatch": minibatch,
        "vf_coef": vf_coef,
        "anchor_coef": anchor_coef,
        "holdout": holdout,
        "history": history,
        "metrics": history[-1] if history else {},
    }
    extra = dict(ckpt.get("extra") or {})
    extra["distill"] = distill
    checkpoint.save(out, model.cpu(), iteration=int(ckpt.get("iteration", 0)), args=dict(ckpt.get("args") or {}), extra=extra)
    print(f"saved {out}", flush=True)
    return history


def main(argv: list[str] | None = None) -> None:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    g = sub.add_parser("generate", help="play lobbies with searching seats and record their targets")
    g.add_argument("--model", required=True)
    g.add_argument("--lobbies", type=int, required=True)
    g.add_argument("--search-seats", type=int, default=2)
    g.add_argument("--out", required=True)
    g.add_argument("--workers", type=int, default=default_workers())
    g.add_argument("--seed", type=int, default=9_000_000)
    g.add_argument("--chunk", type=int, default=50, help="lobbies per shard")
    defaults = SearchSpec()
    g.add_argument("--search-m", type=int, default=defaults.top_m)
    g.add_argument("--search-sims", type=int, default=defaults.sims)
    t = sub.add_parser("train", help="fit the network to the search's targets")
    t.add_argument("--init", required=True)
    t.add_argument("--data", required=True)
    t.add_argument("--out", required=True)
    t.add_argument("--epochs", type=int, default=2)
    t.add_argument("--lr", type=float, default=1e-5)
    t.add_argument("--minibatch", type=int, default=2048)
    t.add_argument("--vf-coef", type=float, default=0.5)
    t.add_argument("--anchor-coef", type=float, default=0.1)
    t.add_argument("--holdout", type=float, default=0.05)
    t.add_argument("--threads", type=int, default=default_threads())
    t.add_argument("--seed", type=int, default=0)
    add_device_arg(t)
    args = p.parse_args(argv)
    if args.cmd == "generate":
        generate(
            args.model, args.out, args.lobbies, args.search_seats, args.workers, args.seed, args.chunk,
            SearchSpec(top_m=args.search_m, sims=args.search_sims),
        )
    else:
        torch.set_num_threads(args.threads)
        run_train(
            args.init, args.data, args.out, args.epochs, args.lr, args.minibatch, args.vf_coef,
            args.anchor_coef, args.holdout, args.device, args.seed,
        )


if __name__ == "__main__":
    main()
