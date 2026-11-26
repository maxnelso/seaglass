"""Rate any agent on a fixed ladder of rule bots, and compare two ratings.

    python -m seaglass_rl.ladder rate runs/solo/latest.pt                 # fast ladder, 500 lobbies
    python -m seaglass_rl.ladder rate 9kgamerbot --out results/9k.json
    python -m seaglass_rl.ladder rate runs/solo/latest.pt --ladder slow   # adds a SovereignBot seat; slow
    python -m seaglass_rl.ladder compare results/a.json results/b.json
    python -m seaglass_rl.ladder rate runs/solo/latest.pt --search       # search inside the turn (search.py)
    python -m seaglass_rl.ladder versus a.pt b.pt --search-a --games 200  # head to head, 4 seats each

The agent takes 2 of the 8 seats of every Solo lobby. The other 6 are the ladder's anchors, rule
bots that must not change between runs. The seeds are fixed, and the seat shuffle does not depend on
the agent, so every agent rated on the same ladder and seed plays the same lobbies in the same
seats. That makes comparisons paired.

Average place: 4.5 is level with the field, lower is better. The standard errors are clustered by
lobby, since the agent's two seats in a lobby are not independent. A difference inside 2 standard
errors is noise. Results record the commit and a fingerprint of the engine build.

`versus` plays two agents against each other instead: each takes 4 of the 8 seats of every Solo lobby
(seats shuffled with the seed alone). It is the most sensitive check of one agent against another, for
example the same checkpoint with and without search. 4.5 is level.

Besides placement, a result reports the agent's behavior by turn (Tavern Tier, Gold left, actions)
and its probe score by turn (`probe.py`): the win rate of its end-of-turn board against reference
boards from the same turn.
"""

from __future__ import annotations

import argparse
import dataclasses
import hashlib
import json
import math
import os
import random
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import numpy as np

from .device import default_workers
from .positioning import PositionSpec
from .play import BOTS, GameSpec, LobbyConfig, Player, PlayOptions
from .probe import ProbeSpec, library_for, library_summary
from .search import SearchSpec

AGENT = "agent"
KINDS = (
    "buy",
    "sell",
    "refresh",
    "upgrade_tavern",
    "play",
    "reposition",
    "toggle_freeze",
    "hero_power",
    "choose_discover",
)
TIER_TURNS = (3, 5, 7, 9, 11)


@dataclasses.dataclass(frozen=True)
class Ladder:
    """The agent's seats and the anchors that fill the rest."""

    name: str
    anchors: tuple[str, ...]  # the anchor seats (bot names from `play.BOTS`)
    agent_seats: int = 2
    games: int = 500
    seed: int = 5_000_000


LADDERS = {
    "fast": Ladder(
        "fast",
        ("9kgamerbot", "9kgamerbot", "9kgamerbot-tempo", "9kgamerbot-tempo", "heuristic", "heuristic"),
        games=500,
        seed=5_000_000,
    ),
    "slow": Ladder(
        "slow",
        ("sovereign", "9kgamerbot", "9kgamerbot", "9kgamerbot-tempo", "9kgamerbot-tempo", "heuristic"),
        games=100,
        seed=6_000_000,
    ),
}


def ladder_specs(
    ladder: Ladder, games: int, seed: int, agent_controller: str, probe: bool
) -> list[tuple[GameSpec, tuple[int, ...]]]:
    """The ladder's games, each with the agent's units. The seating is shuffled with the `AGENT`
    token, before the agent's controller is put in, so it never depends on the agent."""
    if ladder.agent_seats + len(ladder.anchors) != 8:
        raise ValueError(f"a ladder fills 8 seats, got {ladder.agent_seats} + {len(ladder.anchors)}")
    for a in ladder.anchors:
        if a not in BOTS:
            raise ValueError(f"anchor {a!r} is not a bot")
    rng = random.Random(seed)
    out = []
    for g in range(games):
        seats = [AGENT] * ladder.agent_seats + list(ladder.anchors)
        rng.shuffle(seats)
        agent_units = tuple(u for u, s in enumerate(seats) if s == AGENT)
        controllers = [agent_controller if s == AGENT else s for s in seats]
        spec = GameSpec(seed=seed + g, controllers=controllers, probe_units=agent_units if probe else ())
        out.append((spec, agent_units))
    return out


def _saved_step_cap(args: dict) -> int | None:
    """A checkpoint's saved step cap: 30 if missing (saved before the turn timer), else None or an int."""
    if "max_steps_per_turn" not in args:
        return 30
    cap = args["max_steps_per_turn"]
    return None if cap is None else int(cap)


def checkpoint_options(agent: Any) -> PlayOptions:
    """The move rules a checkpoint was trained with (`train_lobby.play_options`: the step cap, the
    end-of-turn step, Reposition, Freeze, pruning), so it is rated under the same rules. Defaults for
    a bot, a `(config, state_dict)` pair, or a checkpoint saved without its training arguments."""
    if not isinstance(agent, str) or agent in BOTS or not os.path.isfile(agent):
        return PlayOptions()
    from . import checkpoint

    _, ckpt = checkpoint.load(agent)
    a = ckpt.get("args") or {}
    return PlayOptions(
        max_steps_per_turn=_saved_step_cap(a),
        positioning_phase=not a.get("no_positioning_phase", False),
        allow_reposition=not a.get("no_reposition", False),
        allow_freeze=not a.get("no_freeze", False),
        prune_dominated=bool(a.get("prune", False)),
        auto_position=PositionSpec() if a.get("auto_position", False) else None,
    )


# -- Rating ---------------------------------------------------------------------------------------


def _git(*args: str) -> str | None:
    try:
        out = subprocess.run(
            ["git", *args],
            cwd=Path(__file__).resolve().parent,
            capture_output=True,
            text=True,
            timeout=20,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    return out.stdout if out.returncode == 0 else None


def _commit_info() -> tuple[str | None, bool | None]:
    head = _git("rev-parse", "HEAD")
    status = _git("status", "--porcelain", "--untracked-files=no")
    return (head.strip() if head else None), (bool(status.strip()) if status is not None else None)


# The source files of the ladder's anchor bots. They are pinned (Stage 5: `bots/` is frozen as a
# reference), so a change to any of them shows up as a different `anchors` fingerprint.
_ANCHOR_SOURCES = (
    "rl/seaglass_rl/bots.py",
    "rl/seaglass_rl/gamerbot.py",
    "bots/seaglass_bots/sovereign_search.py",
    "bots/seaglass_bots/sovereign_evaluator.py",
    "bots/seaglass_bots/sovereign_knowledge.py",
    "bots/seaglass_bots/sovereign_belief.py",
    "bots/seaglass_bots/sovereign_model.py",
    "bots/seaglass_bots/sovereign_positioning.py",
)


def _anchors_fingerprint() -> str | None:
    """The first 12 hex characters of the sha256 of the anchor bots' sources (`_ANCHOR_SOURCES`)."""
    root = Path(__file__).resolve().parents[2]
    h = hashlib.sha256()
    try:
        for rel in _ANCHOR_SOURCES:
            h.update(rel.encode())
            h.update((root / rel).read_bytes())
    except OSError:
        return None
    return h.hexdigest()[:12]


def _engine_fingerprint() -> str | None:
    """The first 12 hex characters of the sha256 of the compiled engine extension."""
    try:
        import seaglass

        path = None
        try:
            import importlib

            path = getattr(importlib.import_module("seaglass.seaglass"), "__file__", None)
        except ImportError:
            pass
        if path is None:
            found = sorted(Path(seaglass.__file__).resolve().parent.glob("*.so")) + sorted(
                Path(seaglass.__file__).resolve().parent.glob("*.pyd")
            )
            path = str(found[0]) if found else None
        if path is None:
            return None
        h = hashlib.sha256()
        with open(path, "rb") as f:
            for block in iter(lambda: f.read(1 << 20), b""):
                h.update(block)
        return h.hexdigest()[:12]
    except Exception:
        return None


def _place_stats(per_lobby: list[list[int]]) -> dict[str, Any]:
    """Place statistics over seats, with the standard error clustered by lobby."""
    flat = np.asarray([p for lobby in per_lobby for p in lobby], dtype=np.float64)
    means = np.asarray([np.mean(lobby) for lobby in per_lobby], dtype=np.float64)
    n = len(means)
    stderr = float(means.std(ddof=1) / math.sqrt(n)) if n > 1 else 0.0
    return {
        "seat_games": int(len(flat)),
        "avg_place": float(flat.mean()),
        "stderr": stderr,
        "top4": float((flat <= 4).mean()),
        "first": float((flat == 1).mean()),
        "places": {k: int((flat == k).sum()) for k in range(1, 9)},
    }


def _turn_table(seats: list[list[Any]], with_probe: bool) -> dict[int, dict[str, Any]]:
    """Per-turn means over a group's seats; `seats` is one list of `TurnStat` per seat-game."""
    by_turn: dict[int, list[Any]] = {}
    for stats in seats:
        for s in stats:
            by_turn.setdefault(s.turn, []).append(s)
    n_seats = len(seats)
    table: dict[int, dict[str, Any]] = {}
    for turn in range(1, max(by_turn, default=0) + 1):
        rows = by_turn.get(turn, [])
        n = len(rows)
        if not n:
            continue
        kinds = {k: float(np.mean([r.actions.get(k, 0) for r in rows])) for k in KINDS}
        entry: dict[str, Any] = {
            "n": n,
            "alive": n / n_seats,
            "tier": float(np.mean([r.tier for r in rows])),
            "gold_left": float(np.mean([r.gold_left for r in rows])),
            "health": float(np.mean([r.health for r in rows])),
            "board": float(np.mean([r.board for r in rows])),
            "actions": float(np.mean([sum(r.actions.values()) for r in rows])),
            "kinds": kinds,
        }
        if with_probe:
            vals = [r.probe for r in rows if r.probe is not None]
            if vals:
                entry["probe"] = float(np.mean(vals))
                entry["probe_se"] = float(np.std(vals) / math.sqrt(len(vals)))
        table[turn] = entry
    return table


def _behavior(seats: list[list[Any]], table: dict[int, dict[str, Any]]) -> dict[str, Any]:
    rows = [s for stats in seats for s in stats]
    return {
        "tier_at": {t: table[t]["tier"] if t in table else None for t in TIER_TURNS},
        "gold_left": float(np.mean([r.gold_left for r in rows])) if rows else None,
        "actions_per_turn": float(np.mean([sum(r.actions.values()) for r in rows])) if rows else None,
    }


def rate(
    agent: Any,
    ladder: str = "fast",
    games: int | None = None,
    seed: int | None = None,
    config: LobbyConfig | None = None,
    player: Player | None = None,
    options: PlayOptions | None = None,
    probe: ProbeSpec | None = ProbeSpec(),
    workers: int | None = None,
    replay_dir: str | None = None,
    max_replays: int = 4,
    search: SearchSpec | None = None,
) -> dict[str, Any]:
    """Rate `agent` (a bot name, a checkpoint path or a `(model_config, state_dict)` pair) on the
    named ladder. Returns a JSON-serializable result (see the module docstring and `format_report`).
    With `search`, the agent's model seats search inside their turns (`search.py`); a bot can't."""
    lad = LADDERS[ladder]
    config = config or LobbyConfig()
    if config.duos or config.players != 8:
        raise ValueError("the ladders are 8-player Solo lobbies")
    games = lad.games if games is None else games
    seed = lad.seed if seed is None else seed
    sources: dict[str, Any] = {}
    if isinstance(agent, str) and agent in BOTS:
        if search is not None:
            raise ValueError(f"{agent!r} is a bot: only a model can search")
        controller, agent_label = agent, agent
    else:
        controller = AGENT
        sources[AGENT] = agent
        agent_label = "<state_dict>" if isinstance(agent, (tuple, list)) else str(agent)
    pairs = ladder_specs(lad, games, seed, controller, probe is not None)
    specs = [s for s, _ in pairs]
    agent_units = {s.seed: u for s, u in pairs}
    options = dataclasses.replace(options or PlayOptions(), turn_stats=True, probe=probe)
    if replay_dir:
        options = dataclasses.replace(options, keep_replays=True)
    if search is not None:
        options = dataclasses.replace(options, search=search, search_controllers=(AGENT,))

    t0 = time.time()
    own_player = player is None
    if player is None:
        player = Player(workers if workers is not None else default_workers())
    try:
        results = player.play(config, specs, sources, options, record=(), seed=seed)
    finally:
        if own_player:
            player.close()
    by_seed = {r.seed: r for r in results}
    results = [by_seed[s.seed] for s in specs]
    if replay_dir:
        for r in results[:max_replays]:
            r.save_replay(os.path.join(replay_dir, f"solo_{r.seed}.replay.pb"))

    # Group the seats: the agent's, and each anchor's (in a Solo lobby, a unit is a player).
    places: dict[str, list[list[int]]] = {}
    seat_stats: dict[str, list[list[Any]]] = {}
    for r in results:
        mine = agent_units[r.seed]
        lobby: dict[str, list[int]] = {}
        for u in range(config.units):
            group = AGENT if u in mine else r.controllers[u]
            lobby.setdefault(group, []).append(int(r.placements[u]))
            seat_stats.setdefault(group, []).append(r.turn_stats[u])
        for group, ps in lobby.items():
            places.setdefault(group, []).append(ps)
    commit, dirty = _commit_info()
    by_turn: dict[str, Any] = {}
    behavior: dict[str, Any] = {}
    for group, seats in seat_stats.items():
        table = _turn_table(seats, with_probe=group == AGENT)
        by_turn[group] = table
        behavior[group] = _behavior(seats, table)
    anchors = sorted(g for g in places if g != AGENT)
    return {
        "meta": {
            "ladder": lad.name,
            "anchors": list(lad.anchors),
            "agent_seats": lad.agent_seats,
            "games": games,
            "seed": seed,
            "agent": agent_label,
            "commit": commit,
            "dirty": dirty,
            "engine": _engine_fingerprint(),
            "anchors_fingerprint": _anchors_fingerprint(),
            "config": dataclasses.asdict(config),
            "search": dataclasses.asdict(search) if search is not None else None,
            "probe": dataclasses.asdict(probe) if probe is not None else None,
            "probe_library": library_summary(library_for(probe, config)) if probe is not None else None,
            "created": datetime.now(timezone.utc).isoformat(timespec="seconds"),
            "seconds": round(time.time() - t0, 1),
        },
        "agent": _place_stats(places[AGENT]),
        "anchors": {a: _place_stats(places[a]) for a in anchors},
        "by_turn": {AGENT: by_turn[AGENT], **{a: by_turn[a] for a in anchors}},
        "behavior": {AGENT: behavior[AGENT], **{a: behavior[a] for a in anchors}},
        "lobbies": [{"seed": r.seed, "agent_places": [int(r.placements[u]) for u in agent_units[r.seed]]} for r in results],
    }


# -- Reports --------------------------------------------------------------------------------------


def _turns(table: dict[Any, Any]) -> dict[int, dict[str, Any]]:
    """A by-turn table with int keys (JSON turns them into strings)."""
    return {int(k): v for k, v in table.items()}


def _line(name: str, s: dict[str, Any]) -> str:
    return (
        f"{name}: avg place {s['avg_place']:.2f} ± {2 * s['stderr']:.2f} (2σ) | top 4 {s['top4']:.0%} | "
        f"1st {s['first']:.0%} | {s['seat_games']} seat-games"
    )


def _mean(entries: list[dict[str, Any]], key: str) -> float:
    w = sum(e["n"] for e in entries)
    return sum(e[key] * e["n"] for e in entries) / w if w else float("nan")


def _search_label(s: dict[str, Any]) -> str:
    return f"search m={s['top_m']} sims={s['sims']} T={s['temperature']}" + (" gumbel" if s["gumbel_noise"] else "")


def format_report(res: dict[str, Any]) -> str:
    meta = res["meta"]
    commit = (meta["commit"] or "?")[:8] + (" (dirty)" if meta.get("dirty") else "")
    lines = [
        f"{meta['ladder']} ladder | {meta['agent']} | {meta['games']} lobbies | seed {meta['seed']} | "
        f"commit {commit} | engine {meta['engine'] or '?'} | {meta['seconds']:.0f}s"
        + (f" | {_search_label(meta['search'])}" if meta.get("search") else ""),
        _line("agent", res["agent"]),
    ]
    for name, s in res["anchors"].items():
        lines.append(_line("  " + name, s))
    tables = {k: _turns(v) for k, v in res["by_turn"].items()}
    agent = tables[AGENT]
    anchors = [t for k, t in tables.items() if k != AGENT]
    has_probe = any("probe" in e for e in agent.values())
    lines.append("turn alive  tier  gold health board  acts buys rolls lvls" + ("  probe" if has_probe else "") + "  | anchors' tier")

    def row(label: str, entries: list[dict[str, Any]], anc: list[dict[str, Any]]) -> str:
        n_all = sum(e["n"] for e in entries)
        probes = [e for e in entries if "probe" in e]
        probe = ""
        if has_probe:
            probe = f"  {_mean(probes, 'probe'):5.2f}" if probes else "      -"
        alive = max(e["alive"] for e in entries)
        kinds = lambda k: sum(e["kinds"][k] * e["n"] for e in entries) / n_all  # noqa: E731
        return (
            f"{label:>4} {alive:4.0%} {_mean(entries, 'tier'):5.1f} {_mean(entries, 'gold_left'):5.1f} "
            f"{_mean(entries, 'health'):6.1f} {_mean(entries, 'board'):5.1f} {_mean(entries, 'actions'):5.1f} "
            f"{kinds('buy'):4.1f} {kinds('refresh'):5.1f} {kinds('upgrade_tavern'):4.1f}{probe}  | "
            + (f"{_mean(anc, 'tier'):.1f}" if anc else "-")
        )

    shown = [t for t in sorted(agent) if t <= 15]
    for t in shown:
        lines.append(row(str(t), [agent[t]], [a[t] for a in anchors if t in a]))
    later = [t for t in sorted(agent) if t > 15]
    if later:
        lines.append(
            row("16+", [agent[t] for t in later], [a[t] for a in anchors for t in a if t > 15])
        )
    return "\n".join(lines)


def format_summary(res: dict[str, Any]) -> str:
    meta, a = res["meta"], res["agent"]
    parts = [
        f"{meta['ladder']} ladder: avg place {a['avg_place']:.2f} ± {2 * a['stderr']:.2f} (2σ)",
        f"top 4 {a['top4']:.0%}",
        f"1st {a['first']:.0%}",
    ]
    table = _turns(res["by_turn"][AGENT])
    probes = [f"T{t} {table[t]['probe']:.2f}" for t in (4, 7, 10) if t in table and "probe" in table[t]]
    if probes:
        parts.append("probe " + " ".join(probes))
    parts.append(f"{meta['games']} lobbies")
    return " | ".join(parts)


# -- Head to head ---------------------------------------------------------------------------------

RIVAL = "rival"
# The move rules of `PlayOptions`: one set per game, so `versus` needs both agents to share them.
_RULES = ("max_steps_per_turn", "positioning_phase", "allow_reposition", "allow_freeze", "prune_dominated", "auto_position")


def versus(
    a: Any,
    b: Any,
    games: int = 200,
    seed: int = 7_000_000,
    config: LobbyConfig | None = None,
    player: Player | None = None,
    options_a: PlayOptions | None = None,
    options_b: PlayOptions | None = None,
    search_a: SearchSpec | None = None,
    search_b: SearchSpec | None = None,
    probe: ProbeSpec | None = None,
    workers: int | None = None,
) -> dict[str, Any]:
    """Agents `a` and `b` (checkpoint paths or `(model_config, state_dict)` pairs; they may be the
    same checkpoint, with only one searching) take 4 seats each in `games` 8-seat Solo lobbies, the
    seats shuffled with `random.Random(seed)` alone. Returns a JSON-serializable result: `a`'s and
    `b`'s place statistics (4.5 is level; the standard error is clustered by lobby, and since the
    places of a lobby sum to 36, `b`'s mean is `9 - a`'s and the two errors are the same) and the
    places by lobby.

    One `PlayOptions` governs a game, so both agents must play under the same move rules
    (`checkpoint_options` of each, or `options_a`/`options_b`): anything else raises `ValueError`.
    The same holds for the search: if both sides search, they must use the same `SearchSpec`."""
    config = config or LobbyConfig()
    if config.duos or config.players != 8:
        raise ValueError("versus plays 8-player Solo lobbies")
    for x in (a, b):
        if isinstance(x, str) and x in BOTS:
            raise ValueError(f"{x!r} is a bot: versus compares two models")
    opts_a = options_a or checkpoint_options(a)
    opts_b = options_b or checkpoint_options(b)
    diff = [r for r in _RULES if getattr(opts_a, r) != getattr(opts_b, r)]
    if diff:
        raise ValueError(f"a and b play under different move rules ({', '.join(diff)}); versus needs equal rules")
    if search_a is not None and search_b is not None and search_a != search_b:
        raise ValueError("both sides search: they must use the same SearchSpec")
    spec = search_a or search_b
    controllers = tuple(c for c, s in ((AGENT, search_a), (RIVAL, search_b)) if s is not None)
    options = dataclasses.replace(opts_a, search=spec, search_controllers=controllers)
    if probe is not None:
        options = dataclasses.replace(options, turn_stats=True, probe=probe)
    rng = random.Random(seed)
    specs, a_units = [], {}
    for g in range(games):
        seats = [AGENT] * 4 + [RIVAL] * 4
        rng.shuffle(seats)
        units = tuple(u for u, s in enumerate(seats) if s == AGENT)
        specs.append(GameSpec(seed=seed + g, controllers=seats, probe_units=units if probe is not None else ()))
        a_units[seed + g] = units
    sources = {AGENT: a, RIVAL: b}
    label = lambda x: "<state_dict>" if isinstance(x, (tuple, list)) else str(x)  # noqa: E731

    t0 = time.time()
    own_player = player is None
    if player is None:
        player = Player(workers if workers is not None else default_workers())
    try:
        results = player.play(config, specs, sources, options, record=(), seed=seed)
    finally:
        if own_player:
            player.close()
    by_seed = {r.seed: r for r in results}
    results = [by_seed[s.seed] for s in specs]
    places: dict[str, list[list[int]]] = {AGENT: [], RIVAL: []}
    lobbies = []
    for r in results:
        mine = a_units[r.seed]
        pa = [int(r.placements[u]) for u in range(config.units) if u in mine]
        pb = [int(r.placements[u]) for u in range(config.units) if u not in mine]
        places[AGENT].append(pa)
        places[RIVAL].append(pb)
        lobbies.append({"seed": r.seed, "a_places": pa, "b_places": pb})
    commit, dirty = _commit_info()
    out: dict[str, Any] = {
        "meta": {
            "games": games,
            "seed": seed,
            "a": label(a),
            "b": label(b),
            "search_a": dataclasses.asdict(search_a) if search_a is not None else None,
            "search_b": dataclasses.asdict(search_b) if search_b is not None else None,
            "rules": {r: str(getattr(options, r)) for r in _RULES},
            "commit": commit,
            "dirty": dirty,
            "engine": _engine_fingerprint(),
            "anchors_fingerprint": _anchors_fingerprint(),
            "config": dataclasses.asdict(config),
            "created": datetime.now(timezone.utc).isoformat(timespec="seconds"),
            "seconds": round(time.time() - t0, 1),
        },
        "a": _place_stats(places[AGENT]),
        "b": _place_stats(places[RIVAL]),
        "lobbies": lobbies,
    }
    if probe is not None:
        seats = {AGENT: [], RIVAL: []}
        for r in results:
            for u in range(config.units):
                seats[AGENT if u in a_units[r.seed] else RIVAL].append(r.turn_stats[u])
        out["by_turn"] = {"a": _turn_table(seats[AGENT], True), "b": _turn_table(seats[RIVAL], True)}
    return out


def format_versus(res: dict[str, Any]) -> str:
    meta = res["meta"]
    commit = (meta["commit"] or "?")[:8] + (" (dirty)" if meta.get("dirty") else "")

    def name(side: str) -> str:
        s = meta[f"search_{side}"]
        return f"{meta[side]}" + (f" [{_search_label(s)}]" if s else "")

    return "\n".join(
        [
            f"versus | {meta['games']} lobbies, 4 seats each | seed {meta['seed']} | commit {commit} | "
            f"engine {meta['engine'] or '?'} | {meta['seconds']:.0f}s",
            "  " + _line("a", res["a"]) + f"  <- {name('a')}",
            "  " + _line("b", res["b"]) + f"  <- {name('b')}",
            f"a - 4.5: {res['a']['avg_place'] - 4.5:+.2f} ± {2 * res['a']['stderr']:.2f} (2σ); negative: a is better",
        ]
    )


# -- Comparing ------------------------------------------------------------------------------------


def compare(a: dict[str, Any], b: dict[str, Any]) -> dict[str, Any]:
    """The difference in average place, `a - b` (negative: `a` is better). Both must be ratings on
    the same ladder."""
    ma, mb = a["meta"], b["meta"]
    if ma["ladder"] != mb["ladder"] or list(ma["anchors"]) != list(mb["anchors"]):
        raise ValueError("the results are from different ladders")
    diff = a["agent"]["avg_place"] - b["agent"]["avg_place"]
    se = math.sqrt(a["agent"]["stderr"] ** 2 + b["agent"]["stderr"] ** 2)
    out: dict[str, Any] = {
        "ladder": ma["ladder"],
        "a": {"agent": ma["agent"], "avg_place": a["agent"]["avg_place"], "stderr": a["agent"]["stderr"]},
        "b": {"agent": mb["agent"], "avg_place": b["agent"]["avg_place"], "stderr": b["agent"]["stderr"]},
        "diff": diff,
        "se": se,
        "agree": abs(diff) <= 2 * se,
        "paired": None,
    }
    seeds_a = [lob["seed"] for lob in a["lobbies"]]
    seeds_b = [lob["seed"] for lob in b["lobbies"]]
    if ma["seed"] == mb["seed"] and seeds_a == seeds_b and seeds_a:
        d = np.asarray(
            [np.mean(x["agent_places"]) - np.mean(y["agent_places"]) for x, y in zip(a["lobbies"], b["lobbies"])]
        )
        n = len(d)
        pse = float(d.std(ddof=1) / math.sqrt(n)) if n > 1 else 0.0
        out["paired"] = {
            "lobbies": n,
            "diff": float(d.mean()),
            "se": pse,
            "z": float(d.mean() / pse) if pse > 0 else None,
        }
    warnings = []
    for key in ("commit", "dirty", "engine", "anchors_fingerprint"):
        if ma.get(key) != mb.get(key):
            warnings.append(f"{key} differs: {ma.get(key)} vs {mb.get(key)}")
    if ma.get("config") != mb.get("config"):
        warnings.append("lobby configs differ")
    out["warnings"] = warnings
    return out


def format_compare(c: dict[str, Any]) -> str:
    lines = [
        f"{c['ladder']} ladder",
        f"a: {c['a']['agent']}: {c['a']['avg_place']:.2f} ± {2 * c['a']['stderr']:.2f} (2σ)",
        f"b: {c['b']['agent']}: {c['b']['avg_place']:.2f} ± {2 * c['b']['stderr']:.2f} (2σ)",
        f"a - b: {c['diff']:+.2f} ± {2 * c['se']:.2f} (2σ) | "
        + ("within noise (agree)" if c["agree"] else "outside 2σ (differ)"),
    ]
    p = c.get("paired")
    if p:
        z = f"{p['z']:+.1f}" if p["z"] is not None else "n/a"
        lines.append(f"paired over {p['lobbies']} lobbies: {p['diff']:+.2f} ± {2 * p['se']:.2f} (2σ), z {z}")
    lines += [f"warning: {w}" for w in c["warnings"]]
    return "\n".join(lines)


# -- Command line ---------------------------------------------------------------------------------


def _search_args(parser: argparse.ArgumentParser) -> None:
    defaults = SearchSpec()
    parser.add_argument("--search-m", type=int, default=defaults.top_m, help="root actions tried (top_m)")
    parser.add_argument("--search-sims", type=int, default=defaults.sims, help="lines per decision")
    parser.add_argument(
        "--search-temperature", type=float, default=defaults.temperature, help="the policy's temperature inside lines"
    )


def _search_spec(args: argparse.Namespace) -> SearchSpec:
    return SearchSpec(top_m=args.search_m, sims=args.search_sims, temperature=args.search_temperature)


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("rate", help="rate an agent on a ladder")
    r.add_argument("agent", help="a checkpoint, or a bot name")
    r.add_argument("--ladder", choices=sorted(LADDERS), default="fast")
    r.add_argument("--games", type=int, default=None, help="default: the ladder's own size")
    r.add_argument("--seed", type=int, default=None, help="default: the ladder's own seed")
    r.add_argument("--workers", type=int, default=default_workers(), help="default: $SEAGLASS_WORKERS, else half the cores")
    r.add_argument(
        "--auto-position",
        choices=("checkpoint", "on", "off"),
        default="checkpoint",
        help="order the model's board by search (positioning.py): as the checkpoint was trained, or override",
    )
    r.add_argument("--no-probe", action="store_true")
    r.add_argument("--deterministic", action="store_true", help="take the most likely action")
    r.add_argument(
        "--max-steps-per-turn",
        type=int,
        default=None,
        help="default: the checkpoint's training setting (30 for checkpoints saved before the turn timer)",
    )
    r.add_argument("--out", default=None, metavar="FILE.json", help="write the full result here")
    r.add_argument("--json", action="store_true", help="print the full result as JSON")
    r.add_argument("--replays", default=None, metavar="DIR", help="save a few games as replays in DIR")
    r.add_argument("--search", action="store_true", help="the agent searches inside its turns (search.py)")
    _search_args(r)
    v = sub.add_parser("versus", help="two checkpoints head to head, 4 seats each")
    v.add_argument("a", help="a checkpoint")
    v.add_argument("b", help="a checkpoint (may be the same as a)")
    v.add_argument("--games", type=int, default=200)
    v.add_argument("--seed", type=int, default=7_000_000)
    v.add_argument("--workers", type=int, default=default_workers(), help="default: $SEAGLASS_WORKERS, else half the cores")
    v.add_argument("--search-a", action="store_true", help="a searches inside its turns")
    v.add_argument("--search-b", action="store_true", help="b searches inside its turns")
    _search_args(v)
    v.add_argument("--out", default=None, metavar="FILE.json", help="write the full result here")
    v.add_argument("--json", action="store_true", help="print the full result as JSON")
    c = sub.add_parser("compare", help="compare two result files")
    c.add_argument("a")
    c.add_argument("b")
    c.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    if args.cmd == "rate":
        opts = dataclasses.replace(
            checkpoint_options(args.agent),
            deterministic=args.deterministic,
            **({} if args.auto_position == "checkpoint" else {"auto_position": PositionSpec() if args.auto_position == "on" else None}),
            **({"max_steps_per_turn": args.max_steps_per_turn} if args.max_steps_per_turn else {}),
        )
        res = rate(
            args.agent,
            ladder=args.ladder,
            games=args.games,
            seed=args.seed,
            options=opts,
            probe=None if args.no_probe else ProbeSpec(),
            workers=args.workers,
            replay_dir=args.replays,
            search=_search_spec(args) if args.search else None,
        )
        if args.out:
            os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
            with open(args.out, "w", encoding="utf-8") as f:
                json.dump(res, f, indent=1)
        print(json.dumps(res) if args.json else format_report(res))
    elif args.cmd == "versus":
        spec = _search_spec(args)
        res = versus(
            args.a,
            args.b,
            games=args.games,
            seed=args.seed,
            search_a=spec if args.search_a else None,
            search_b=spec if args.search_b else None,
            workers=args.workers,
        )
        if args.out:
            os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
            with open(args.out, "w", encoding="utf-8") as f:
                json.dump(res, f, indent=1)
        print(json.dumps(res) if args.json else format_versus(res))
    else:
        loaded = []
        for path in (args.a, args.b):
            with open(path, encoding="utf-8") as f:
                loaded.append(json.load(f))
        out = compare(*loaded)
        print(json.dumps(out) if args.json else format_compare(out))


if __name__ == "__main__":
    main()
