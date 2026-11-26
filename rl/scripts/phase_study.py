"""How much of a game is already settled, turn by turn: forked rollouts from every turn of bot lobbies.

The study behind `docs/policy-value-by-phase.md`. It needs no network and no GPU: the seats are
rule bots (`play.BOTS`).

For each base lobby, at the start of every turn (the shops are dealt and the pairings known):

- **Forks.** Copy the lobby `--forks` times (`LobbyGame.copy`), deal each copy a different future
  (`LobbyGame.reseed`) and play it to the end with copies of the same bots. One position, many
  outcomes: the spread of a seat's final place across the forks is the part of the outcome that
  the position does not decide, whatever a critic knows.
- **Skips.** Do the same again with one living seat ending this turn at once, on the same seeds.
  The difference in that seat's place is what one turn of play is worth.

By turn it reports:

- `noise_sd`: the SD of a living seat's final place across forks of one position;
- `signal_sd`: the SD, across positions, of a position's expected place (among living seats);
- `ceiling_r2`: signal / (signal + noise) as variances: the most a critic could explain of a
  Monte Carlo placement target at this turn;
- `settled`: the share of the whole lobby's final-place variance (5.25) that is already decided,
  counting seats that are out and games that are over as decided;
- `skip_cost`: the places lost by skipping the turn, with its standard error;
- the players left, the actions and Refreshes per turn, the legal actions per decision (without
  Repositions), how one-sided the turn's fights are, and what a fork costs.

It also groups positions by the number of players left, and for the final two it reports how often
the game is still open.

    python rl/scripts/phase_study.py --games 300 --forks 16 --workers 4 --out docs/results/phase-study/summary.json

The numbers describe games between rule bots. A stronger field will move them.
"""

from __future__ import annotations

import argparse
import copy
import json
import math
import multiprocessing as mp
import random
import statistics
import subprocess
import time
from collections import defaultdict
from pathlib import Path
from typing import Any

import seaglass

from seaglass_rl.actions import END_TURN_INDEX, action_from_index
from seaglass_rl.play import LobbyConfig, Seat, _bot

# A backstop on a bot's actions per phase (`play.py` dropped its own cap for the engine's turn
# timer, which also ends a phase).
BOT_MAX_STEPS_PER_TURN = 100

UNIFORM_PLACE_VARIANCE = 5.25  # of a place drawn uniformly from 1..8
FIGHT_SIMS = 100
# (name, first turn, last turn): the phases the document reports.
PHASES = (("early", 1, 4), ("middle", 5, 9), ("late", 10, 99))


def act_phase(game: seaglass.LobbyGame, bots: list[Any], skip: int | None = None, tally: dict | None = None) -> None:
    """Every player still to act plays their phase, one action each in turn. `skip` ends theirs at
    once. `tally` collects, by turn, the actions taken, the Refreshes among them, and the legal
    actions at each decision (Repositions left out: a full board alone has 42 of them)."""
    views = [Seat(game, p) for p in range(game.num_players)]
    steps = [0] * game.num_players
    while True:
        active = game.active_players()
        if not active:
            return
        for p in active:
            if p == skip or steps[p] >= BOT_MAX_STEPS_PER_TURN:
                game.force_end(p)
                continue
            action = action_from_index(bots[p].act(views[p]))
            if not game.is_legal(p, action):
                action = action_from_index(END_TURN_INDEX)
            if tally is not None and game.phase == "turn":
                row = tally.setdefault(game.turn, {"decisions": 0, "legal": 0, "rolls": 0})
                row["decisions"] += 1
                row["legal"] += sum(1 for a in game.legal_actions(p) if a.kind != "reposition")
                row["rolls"] += action.kind == "refresh"
            game.step(p, action)
            steps[p] += 1


def play_out(game: seaglass.LobbyGame, bots: list[Any], skip: int | None = None) -> list[int]:
    """Play `game` to the end from inside a phase that has begun. Returns every unit's place."""
    while True:
        act_phase(game, bots, skip)
        skip = None
        game.end_phase()
        if game.done:
            return [int(p) for p in game.placements]
        game.begin_phase()


def fight_edges(game: seaglass.LobbyGame, seed: int) -> list[float]:
    """For each pair about to fight: the favourite's win rate minus the underdog's, board against
    board (no Hero Powers or trinkets, as in the Stage 1 probe)."""
    edges = []
    for a, b in game.pairs[0]:
        oa, ob = game.observe(a), game.observe(b)
        if not oa.board or not ob.board:
            continue
        d = seaglass.simulate_batch(
            list(oa.board), list(ob.board), seed, FIGHT_SIMS, hero_tier_a=oa.tavern_tier, hero_tier_b=ob.tavern_tier
        )
        edges.append(abs(d.a_win_rate - d.b_win_rate))
    return edges


def study_game(job: tuple[int, tuple[str, ...], int, bool]) -> dict:
    """One base lobby: its forks and skips at the start of every turn."""
    seed, controllers, forks, skips = job
    game = LobbyConfig().make(seed, meta={"source": "phase_study"})
    units = game.num_units
    bots = [_bot(controllers[game.unit_of(p)], seed * 16 + p) for p in range(game.num_players)]
    pick = random.Random(seed)
    snapshots: list[dict] = []
    tally: dict[int, dict] = {}
    fights: dict[int, list[float]] = {}
    game.begin_phase()
    while True:
        if game.phase == "turn":
            alive = list(game.alive)
            base = (seed * 1_000_003 + game.turn * 10_007) % (1 << 62)
            start = time.perf_counter()
            places = []
            for k in range(forks):
                fork = game.copy()
                fork.reseed(base + k)
                places.append(play_out(fork, copy.deepcopy(bots)))
            seconds = (time.perf_counter() - start) / max(forks, 1)
            snap = {"turn": game.turn, "alive": alive, "places": places, "fork_seconds": seconds}
            if skips:
                unit = pick.choice([u for u in range(units) if alive[u]])
                skipped = []
                for k in range(forks):
                    fork = game.copy()
                    fork.reseed(base + k)
                    skipped.append(play_out(fork, copy.deepcopy(bots), skip=unit)[unit])
                snap["skip_unit"] = unit
                snap["skip_places"] = skipped
            snapshots.append(snap)
        act_phase(game, bots, tally=tally)
        if game.phase == "turn":
            fights[game.turn] = fight_edges(game, seed + game.turn)
        game.end_phase()
        if game.done:
            break
        game.begin_phase()
    return {
        "seed": seed,
        "turns": game.turn,
        "placements": [int(p) for p in game.placements],
        "snapshots": snapshots,
        "tally": tally,
        "fights": fights,
    }


def _var(xs: list[float]) -> float:
    return statistics.variance(xs) if len(xs) > 1 else 0.0


def _mean(xs: list[float]) -> float:
    return sum(xs) / len(xs) if xs else float("nan")


def _se(xs: list[float]) -> float:
    return math.sqrt(_var(xs) / len(xs)) if len(xs) > 1 else float("nan")


def position_stats(snaps: list[dict]) -> dict:
    """Noise and signal among the living seats of `snaps` (law of total variance: the pooled
    variance of every fork sample, less the mean within-position variance)."""
    within, pooled = [], []
    for s in snaps:
        for u, alive in enumerate(s["alive"]):
            if not alive:
                continue
            samples = [fork[u] for fork in s["places"]]
            within.append(_var(samples))
            pooled.extend(samples)
    if not within:
        return {}
    noise = _mean(within)
    total = statistics.pvariance(pooled)
    signal = max(total - noise, 0.0)
    return {
        "positions": len(snaps),
        "seats": len(within),
        "noise_sd": math.sqrt(noise),
        "signal_sd": math.sqrt(signal),
        "ceiling_r2": signal / total if total else float("nan"),
    }


def summarize(games: list[dict], forks: int) -> dict:
    max_turn = max(g["turns"] for g in games)
    by_turn: dict[int, list[dict]] = defaultdict(list)
    by_left: dict[int, list[dict]] = defaultdict(list)
    for g in games:
        for s in g["snapshots"]:
            by_turn[s["turn"]].append(s)
            by_left[sum(s["alive"])].append(s)

    turns = []
    for t in range(1, max_turn + 1):
        snaps = by_turn.get(t, [])
        row: dict[str, Any] = {"turn": t, "games_reaching": len(snaps)}
        row.update(position_stats(snaps))
        # The share of the lobby's final-place variance already decided, over every base game: a
        # seat that is out, and a game that is over, have nothing left to decide.
        undecided = 0.0
        for s in snaps:
            for u in range(len(s["alive"])):
                undecided += _var([fork[u] for fork in s["places"]])
        row["settled"] = 1.0 - undecided / (len(games) * 8 * UNIFORM_PLACE_VARIANCE)
        if snaps:
            row["players_left"] = _mean([sum(s["alive"]) for s in snaps])
            row["fork_seconds"] = _mean([s["fork_seconds"] for s in snaps])
            costs = [
                _mean(s["skip_places"]) - _mean([fork[s["skip_unit"]] for fork in s["places"]])
                for s in snaps
                if "skip_unit" in s
            ]
            if costs:
                row["skip_cost"] = _mean(costs)
                row["skip_cost_se"] = _se(costs)
        decisions = sum(g["tally"].get(t, {}).get("decisions", 0) for g in games)
        legal = sum(g["tally"].get(t, {}).get("legal", 0) for g in games)
        seat_turns = sum(sum(s["alive"]) for s in snaps)
        if decisions and seat_turns:
            row["actions_per_turn"] = decisions / seat_turns
            row["rolls_per_turn"] = sum(g["tally"].get(t, {}).get("rolls", 0) for g in games) / seat_turns
            row["legal_per_decision"] = legal / decisions
        edges = [e for g in games for e in g["fights"].get(t, [])]
        if edges:
            row["fight_edge"] = _mean(edges)
            row["fights_decided"] = _mean([1.0 if e >= 0.8 else 0.0 for e in edges])
        turns.append(row)

    phases = []
    for name, first, last in PHASES:
        snaps = [s for t in range(first, min(last, max_turn) + 1) for s in by_turn.get(t, [])]
        row = {"phase": name, "turns": [first, min(last, max_turn)]}
        row.update(position_stats(snaps))
        tallies = [g["tally"].get(t, {}) for g in games for t in range(first, min(last, max_turn) + 1)]
        decisions = sum(x.get("decisions", 0) for x in tallies)
        seat_turns = sum(sum(s["alive"]) for s in snaps)
        if decisions and seat_turns:
            row["actions_per_turn"] = decisions / seat_turns
            row["rolls_per_turn"] = sum(x.get("rolls", 0) for x in tallies) / seat_turns
            row["legal_per_decision"] = sum(x.get("legal", 0) for x in tallies) / decisions
        phases.append(row)

    # The ceiling for a critic scored over every decision of every game, as a PPO log scores it:
    # each living seat of a position counts once per action it takes that turn.
    weights = {r["turn"]: r.get("actions_per_turn", 0.0) for r in turns}
    w_sum = w_noise = w_x = w_xx = 0.0
    for t, snaps in by_turn.items():
        for s in snaps:
            for u, alive in enumerate(s["alive"]):
                if alive:
                    samples = [fork[u] for fork in s["places"]]
                    w_sum += weights[t]
                    w_noise += weights[t] * _var(samples)
                    w_x += weights[t] * _mean(samples)
                    w_xx += weights[t] * _mean([x * x for x in samples])
    pooled_total = w_xx / w_sum - (w_x / w_sum) ** 2
    pooled_ceiling = 1.0 - (w_noise / w_sum) / pooled_total

    left = []
    for n in sorted(by_left, reverse=True):
        snaps = by_left[n]
        row = {"players_left": n}
        row.update(position_stats(snaps))
        row["fork_seconds"] = _mean([s["fork_seconds"] for s in snaps])
        row["turn"] = _mean([s["turn"] for s in snaps])
        left.append(row)

    # The final two: at the first turn with two players left, the favourite's chance to win. The
    # final four: at the first turn with four or fewer left, the seats whose place no fork changes.
    favourites = []
    fixed = []
    lengths = defaultdict(list)
    for g in games:
        seen = defaultdict(int)
        first_two = first_four = None
        for s in g["snapshots"]:
            n = sum(s["alive"])
            seen[n] += 1
            if n == 2 and first_two is None:
                first_two = s
            if n <= 4 and first_four is None:
                first_four = s
        if first_four is not None:
            for u, alive in enumerate(first_four["alive"]):
                if alive:
                    fixed.append(1.0 if len({fork[u] for fork in first_four["places"]}) == 1 else 0.0)
        for n in (4, 3, 2):
            lengths[n].append(sum(c for k, c in seen.items() if k <= n))
        if first_two is not None:
            pair = [u for u, a in enumerate(first_two["alive"]) if a]
            wins = sum(1 for fork in first_two["places"] if fork[pair[0]] < fork[pair[1]])
            favourites.append(max(wins, forks - wins) / forks)
    final_two = {
        "games": len(favourites),
        "favourite_win_rate": _mean(favourites),
        "open": _mean([1.0 if f < 0.75 else 0.0 for f in favourites]),
        "leaning": _mean([1.0 if 0.75 <= f < 0.95 else 0.0 for f in favourites]),
        "decided": _mean([1.0 if f >= 0.95 else 0.0 for f in favourites]),
    }
    return {
        "games": len(games),
        "forks": forks,
        "game_turns": {
            "mean": _mean([g["turns"] for g in games]),
            "min": min(g["turns"] for g in games),
            "max": max_turn,
        },
        "turns_with_at_most": {str(n): _mean(v) for n, v in lengths.items()},
        "pooled_ceiling_r2": pooled_ceiling,
        "by_phase": phases,
        "by_turn": turns,
        "by_players_left": left,
        "final_four": {"seats": len(fixed), "seats_fixed": _mean(fixed)},
        "final_two": final_two,
    }


def _commit() -> tuple[str, bool | None]:
    """The repository's commit, and whether the working tree had uncommitted changes."""
    root = Path(__file__).resolve().parents[2]

    def git(*args: str) -> str:
        return subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, check=True).stdout.strip()

    try:
        return git("rev-parse", "--short", "HEAD"), bool(git("status", "--porcelain"))
    except Exception:
        return "unknown", None


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--games", type=int, default=300, help="base lobbies")
    ap.add_argument("--forks", type=int, default=16, help="rollouts per position")
    ap.add_argument("--seed", type=int, default=7_000_000)
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument(
        "--field",
        default="9kgamerbot,9kgamerbot-tempo",
        help="the bots, dealt to the 8 seats in rotation",
    )
    ap.add_argument("--no-skips", action="store_true", help="leave out the skipped-turn forks")
    ap.add_argument("--out", type=Path, default=None, help="write the summary here (JSON)")
    ap.add_argument("--raw", type=Path, default=None, help="write every fork's places here (JSON)")
    args = ap.parse_args()

    field = tuple(args.field.split(","))
    controllers = tuple(field[u % len(field)] for u in range(8))
    jobs = [(args.seed + g, controllers, args.forks, not args.no_skips) for g in range(args.games)]
    commit, dirty = _commit()  # read before the games: the code that plays them
    start = time.perf_counter()
    if args.workers > 1:
        with mp.get_context("spawn").Pool(args.workers) as pool:
            games = pool.map(study_game, jobs, chunksize=1)
    else:
        games = [study_game(j) for j in jobs]
    summary = summarize(games, args.forks)
    summary.update(
        {
            "field": list(controllers),
            "seed": args.seed,
            "commit": commit,
            "dirty": dirty,
            "wall_seconds": time.perf_counter() - start,
            "workers": args.workers,
        }
    )
    if args.raw:
        args.raw.parent.mkdir(parents=True, exist_ok=True)
        args.raw.write_text(json.dumps(games))
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(summary, indent=1) + "\n")

    print(f"{summary['games']} lobbies x {args.forks} forks, {summary['wall_seconds']:.0f} s")
    print("turn games left  noise signal ceilR2 settled  skip_cost      acts legal edge  fork_s")
    for r in summary["by_turn"]:
        if "noise_sd" not in r:
            continue
        skip = f"{r['skip_cost']:+.2f}±{r['skip_cost_se']:.2f}" if "skip_cost" in r else "     -    "
        print(
            f"{r['turn']:4d} {r['games_reaching']:5d} {r['players_left']:4.1f} {r['noise_sd']:6.2f} "
            f"{r['signal_sd']:6.2f} {r['ceiling_r2']:6.2f} {r['settled']:7.2f}  {skip:>11}  "
            f"{r.get('actions_per_turn', float('nan')):6.1f} {r.get('legal_per_decision', float('nan')):5.1f} "
            f"{r.get('fight_edge', float('nan')):4.2f} {r['fork_seconds']:6.3f}"
        )
    print(f"critic ceiling over every decision: {summary['pooled_ceiling_r2']:.2f}")
    print("phase: turns, noise, signal, ceiling, actions per turn, rolls per turn, legal per decision")
    for r in summary["by_phase"]:
        if "noise_sd" in r:
            print(
                f"  {r['phase']:6s} {r['turns'][0]:2d}-{r['turns'][1]:2d} {r['noise_sd']:.2f} {r['signal_sd']:.2f} "
                f"{r['ceiling_r2']:.2f} {r.get('actions_per_turn', float('nan')):5.1f} "
                f"{r.get('rolls_per_turn', float('nan')):4.1f} {r.get('legal_per_decision', float('nan')):5.1f}"
            )
    print("final four:", summary["final_four"])
    print("players left: noise, signal, fork seconds")
    for r in summary["by_players_left"]:
        if "noise_sd" in r:
            print(f"  {r['players_left']}: {r['noise_sd']:.2f} {r['signal_sd']:.2f} {r['fork_seconds']:.3f}")
    print("final two:", summary["final_two"])


if __name__ == "__main__":
    main()
