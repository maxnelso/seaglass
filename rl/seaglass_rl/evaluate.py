"""Evaluate a policy by its average placement in full lobbies against a fixed pool of opponents.

    python -m seaglass_rl.evaluate runs/solo/latest.pt                      # vs HeuristicBots, Solo
    python -m seaglass_rl.evaluate runs/duos/latest.pt --duos --games 200
    python -m seaglass_rl.evaluate runs/a/latest.pt --opponents heuristic,runs/b/latest.pt

In each game the agent controls `--agent-units` units (Solo players or Duos teams) at random positions.
The opponents (`heuristic`, `random` or checkpoint paths) fill the other units, cycling through the
list. Playing as well as the field gives an average place of `(units + 1) / 2`: 4.5 in an 8-player
Solo lobby, 2.5 in Duos. Lower is better.
"""

from __future__ import annotations

import argparse
import dataclasses
import json
import os
import math
import random
import time
from typing import Any

import numpy as np

from .play import LobbyConfig, BOTS, GameSpec, Player, PlayOptions

AGENT = "agent"


def evaluate(
    agent: Any,
    opponents: list[str],
    config: LobbyConfig,
    games: int = 64,
    agent_units: int = 1,
    seed: int = 1_000_000,
    player: Player | None = None,
    options: PlayOptions | None = None,
    replay_dir: str | None = None,
    max_replays: int = 4,
) -> dict[str, Any]:
    """`agent` (a checkpoint path, `(config, state_dict)` or a bot name) against `opponents`.

    Returns the agent's average place (with its standard error), its top-half and first-place rates,
    the place distribution and the average game length. With `replay_dir`, the first `max_replays`
    games are saved there as replays (`match_replay tui <file>`).
    """
    rng = random.Random(seed)
    sources: dict[str, Any] = {}
    agent_name = agent if isinstance(agent, str) and agent in BOTS else AGENT
    if agent_name == AGENT:
        sources[AGENT] = agent
    for o in opponents:
        if o not in BOTS:
            sources[o] = o
    specs = []
    for g in range(games):
        units = list(range(config.units))
        mine = set(rng.sample(units, agent_units))
        others = [u for u in units if u not in mine]
        controllers = [""] * config.units
        for u in mine:
            controllers[u] = agent_name
        for i, u in enumerate(others):
            controllers[u] = opponents[(g + i) % len(opponents)]
        specs.append(GameSpec(seed=seed + g, controllers=controllers))
    own_player = player is None
    player = player or Player(0)
    t0 = time.time()
    options = options or PlayOptions()
    if replay_dir:
        options = dataclasses.replace(options, keep_replays=True)
    try:
        results = player.play(config, specs, sources, options, record=(), seed=seed)
    finally:
        if own_player:
            player.close()
    places = [
        r.placements[u] for r in results for u in range(config.units) if r.controllers[u] == agent_name
    ]
    if replay_dir:
        mode = "duos" if config.duos else "solo"
        for r in results[:max_replays]:
            r.save_replay(os.path.join(replay_dir, f"{mode}_{r.seed}.replay.pb"))
    arr = np.asarray(places, dtype=np.float64)
    half = config.units / 2
    return {
        "games": games,
        "units": config.units,
        "avg_place": float(arr.mean()),
        "stderr": float(arr.std(ddof=1) / math.sqrt(len(arr))) if len(arr) > 1 else 0.0,
        "top_half": float((arr <= half).mean()),
        "first": float((arr == 1).mean()),
        "places": {int(k): int((arr == k).sum()) for k in range(1, config.units + 1)},
        "avg_turns": float(np.mean([r.turns for r in results])),
        "seconds": time.time() - t0,
    }


def format_result(name: str, res: dict[str, Any]) -> str:
    return (
        f"{name}: avg place {res['avg_place']:.2f} ± {res['stderr']:.2f} "
        f"(field {(res['units'] + 1) / 2:.1f}) | top half {res['top_half']:.0%} | "
        f"1st {res['first']:.0%} | turns {res['avg_turns']:.1f} | {res['games']} games"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("agent", help="a checkpoint, or `heuristic` / `random`")
    parser.add_argument("--opponents", default="heuristic", help="comma-separated: heuristic, random, checkpoints")
    parser.add_argument("--duos", action="store_true")
    parser.add_argument("--players", type=int, default=8)
    parser.add_argument("--games", type=int, default=100)
    parser.add_argument("--agent-units", type=int, default=1)
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--seed", type=int, default=1_000_000)
    parser.add_argument("--max-turns", type=int, default=30)
    parser.add_argument("--no-heroes", action="store_true")
    parser.add_argument("--deterministic", action="store_true", help="take the most likely action")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--replays", default=None, metavar="DIR", help="save a few games as replays in DIR")
    args = parser.parse_args()

    config = LobbyConfig(
        duos=args.duos, players=args.players, max_turns=args.max_turns, heroes=not args.no_heroes
    )
    opts = PlayOptions(deterministic=args.deterministic)
    player = Player(args.workers)
    try:
        res = evaluate(
            args.agent,
            args.opponents.split(","),
            config,
            games=args.games,
            agent_units=args.agent_units,
            seed=args.seed,
            player=player,
            options=opts,
            replay_dir=args.replays,
        )
    finally:
        player.close()
    print(json.dumps(res) if args.json else format_result(args.agent, res))


if __name__ == "__main__":
    main()
