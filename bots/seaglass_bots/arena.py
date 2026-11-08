"""Arena CLI: Evaluates Python Search Bots vs Baseline League in 8-Player Solo & 4-Team Duos."""

from __future__ import annotations

import argparse
import os
import time
from typing import Dict, List, Protocol

import seaglass

from .baseline import GreedyLevelBot, SemiRandomBot, TempoBot
from .duos import DuosSequentialRolloutBot
from .search import ExpectimaxMctsBot, ShopEpochBeamBot


class LobbyActor(Protocol):
    def act_in_lobby(
        self, lobby: seaglass.LobbyGame, player: int
    ) -> seaglass.TavernAction: ...


MAX_ACTIONS_PER_TURN = 80


def run_solo_benchmark(
    num_games: int = 8,
    bot_kind: str = "beam",
    base_seed: int = 1000,
    save_replay_dir: str | None = None,
) -> None:
    """Pit 2 Search Bots against 6 League Bots (2 SemiRandom, 2 Tempo, 2 GreedyLevel) across games."""
    role_names = [
        f"Search({bot_kind})-0",
        f"Search({bot_kind})-1",
        "SemiRandom-2",
        "SemiRandom-3",
        "Tempo-4",
        "Tempo-5",
        "GreedyLevel-6",
        "GreedyLevel-7",
    ]
    placements_by_group: Dict[str, List[int]] = {
        f"Search({bot_kind})": [],
        "SemiRandom": [],
        "Tempo": [],
        "GreedyLevel": [],
    }
    search_turn_times_ms: List[float] = []
    search_actions_count = 0

    for g_idx in range(num_games):
        seed = base_seed + g_idx * 97
        # Rotate seats across games to eliminate any seat-order bias.
        rot = g_idx % 8
        seat_to_group: Dict[int, str] = {}
        bots: Dict[int, LobbyActor] = {}

        for logical_idx in range(8):
            seat = (logical_idx + rot) % 8
            s_seed = seed + logical_idx * 13
            if logical_idx in (0, 1):
                seat_to_group[seat] = f"Search({bot_kind})"
                if bot_kind == "mcts":
                    bots[seat] = ExpectimaxMctsBot(seed=s_seed)
                else:
                    bots[seat] = ShopEpochBeamBot(seed=s_seed)
            elif logical_idx in (2, 3):
                seat_to_group[seat] = "SemiRandom"
                bots[seat] = SemiRandomBot(seed=s_seed)
            elif logical_idx in (4, 5):
                seat_to_group[seat] = "Tempo"
                bots[seat] = TempoBot(seed=s_seed)
            else:
                seat_to_group[seat] = "GreedyLevel"
                bots[seat] = GreedyLevelBot(seed=s_seed)

        names = [f"{seat_to_group[p]}_p{p}" for p in range(8)]
        lobby = seaglass.LobbyGame(players=8, duos=False, seed=seed, names=names)

        while not lobby.done:
            lobby.begin_phase()
            is_turn = lobby.phase == "turn"
            for p in list(lobby.active_players()):
                is_search = seat_to_group[p].startswith("Search")
                t0 = time.perf_counter() if (is_search and is_turn) else 0.0
                steps = 0
                while not lobby.is_done(p) and steps < MAX_ACTIONS_PER_TURN:
                    act = bots[p].act_in_lobby(lobby, p)
                    if not lobby.is_legal(p, act):
                        lobby.force_end(p)
                        break
                    note = getattr(bots[p], "last_note", None)
                    lobby.step(p, act, note=note)
                    steps += 1
                    if is_search and is_turn:
                        search_actions_count += 1
                if not lobby.is_done(p):
                    lobby.force_end(p)
                if is_search and is_turn:
                    dt_ms = (time.perf_counter() - t0) * 1000.0
                    search_turn_times_ms.append(dt_ms)
            lobby.end_phase()

        places = lobby.placements
        search_places = [
            places[s] for s in range(8) if seat_to_group[s].startswith("Search")
        ]
        if save_replay_dir:
            os.makedirs(save_replay_dir, exist_ok=True)
            best_p = min(x for x in search_places if x is not None)
            out_path = os.path.join(
                save_replay_dir,
                f"solo_{bot_kind}_g{g_idx + 1}_seed{seed}_best{best_p}.replay.pb",
            )
            lobby.set_meta("bot", f"ShopEpochBeamBot({bot_kind})")
            lobby.save(out_path)
            print(f"  -> saved {out_path}")

        for seat, place in enumerate(places):
            if place is not None:
                placements_by_group[seat_to_group[seat]].append(place)
        print(
            f"[Solo Game {g_idx + 1}/{num_games} | turns={lobby.turn}] "
            f"Search({bot_kind}) places={search_places} | all={places}"
        )

    print("\n=== Solo 8-Player League Summary ===")
    for grp, pls in placements_by_group.items():
        avg_p = sum(pls) / max(1, len(pls))
        top4 = 100.0 * sum(1 for x in pls if x <= 4) / max(1, len(pls))
        win1 = 100.0 * sum(1 for x in pls if x == 1) / max(1, len(pls))
        print(
            f"  {grp:<18} | n={len(pls):>2} | Avg Place: {avg_p:.2f} "
            f"| Top-4: {top4:5.1f}% | 1st: {win1:5.1f}%"
        )

    if search_turn_times_ms:
        mean_ms = sum(search_turn_times_ms) / len(search_turn_times_ms)
        p95_ms = sorted(search_turn_times_ms)[int(0.95 * (len(search_turn_times_ms) - 1))]
        print(
            f"\nSearch({bot_kind}) Python Turn Latency: "
            f"mean={mean_ms:.1f} ms/turn, p95={p95_ms:.1f} ms/turn "
            f"({len(search_turn_times_ms)} turns, {search_actions_count} actions)"
        )
    _ = role_names


def run_duos_benchmark(
    num_games: int = 4,
    base_seed: int = 5000,
    save_replay_dir: str | None = None,
) -> None:
    """Pit 1 DuosSequentialRolloutBot team against 3 SemiRandom/Tempo teams (4 teams = 8 players)."""
    team_places: Dict[str, List[int]] = {
        "DuosRollout": [],
        "BaselineTeams": [],
    }
    turn_ms: List[float] = []

    for g_idx in range(num_games):
        seed = base_seed + g_idx * 101
        our_team = g_idx % 4
        bots: Dict[int, LobbyActor] = {}
        names: List[str] = []
        for t in range(4):
            for s in range(2):
                p = 2 * t + s
                if t == our_team:
                    bots[p] = DuosSequentialRolloutBot(seed=seed + p)
                    names.append(f"DuosRollout_p{p}")
                else:
                    bots[p] = SemiRandomBot(seed=seed + p) if t % 2 == 1 else TempoBot(seed=seed + p)
                    names.append(f"Base_p{p}")

        lobby = seaglass.LobbyGame(players=8, duos=True, seed=seed, names=names)
        while not lobby.done:
            lobby.begin_phase()
            is_turn = lobby.phase == "turn"
            for p in list(lobby.active_players()):
                is_ours = lobby.unit_of(p) == our_team
                t0 = time.perf_counter() if (is_ours and is_turn) else 0.0
                steps = 0
                while not lobby.is_done(p) and steps < MAX_ACTIONS_PER_TURN:
                    act = bots[p].act_in_lobby(lobby, p)
                    if not lobby.is_legal(p, act):
                        lobby.force_end(p)
                        break
                    note = getattr(bots[p], "last_note", None)
                    lobby.step(p, act, note=note)
                    steps += 1
                if not lobby.is_done(p):
                    lobby.force_end(p)
                if is_ours and is_turn:
                    turn_ms.append((time.perf_counter() - t0) * 1000.0)
            lobby.end_phase()

        places = lobby.placements
        if save_replay_dir:
            os.makedirs(save_replay_dir, exist_ok=True)
            our_pl = places[our_team]
            out_path = os.path.join(
                save_replay_dir,
                f"duos_rollout_g{g_idx + 1}_seed{seed}_team{our_team}_place{our_pl}.replay.pb",
            )
            lobby.set_meta("bot", "DuosSequentialRolloutBot")
            lobby.save(out_path)
            print(f"  -> saved {out_path}")

        for t, pl in enumerate(places):
            if pl is not None:
                if t == our_team:
                    team_places["DuosRollout"].append(pl)
                else:
                    team_places["BaselineTeams"].append(pl)
        print(
            f"[Duos Game {g_idx + 1}/{num_games} | turns={lobby.turn}] "
            f"DuosRollout team={our_team} place={places[our_team]} | all={places}"
        )

    print("\n=== Duos 4-Team Summary ===")
    for grp, pls in team_places.items():
        avg_p = sum(pls) / max(1, len(pls))
        top2 = 100.0 * sum(1 for x in pls if x <= 2) / max(1, len(pls))
        win1 = 100.0 * sum(1 for x in pls if x == 1) / max(1, len(pls))
        print(
            f"  {grp:<18} | n={len(pls):>2} | Avg Place: {avg_p:.2f} "
            f"| Top-2: {top2:5.1f}% | 1st: {win1:5.1f}%"
        )
    if turn_ms:
        print(f"DuosRollout Python Turn Latency: mean={sum(turn_ms)/len(turn_ms):.1f} ms/turn")


def run_head2head_benchmark(
    num_games: int = 8,
    base_seed: int = 8000,
    save_replay_dir: str | None = None,
) -> None:
    """Pit 2 V2 (Physics + Set-Transformer) vs 2 V1 (Legacy Heuristic) vs 2 Tempo vs 2 GreedyLevel."""
    placements_by_group: Dict[str, List[int]] = {
        "SearchV2": [],
        "SearchV1_Old": [],
        "Tempo": [],
        "GreedyLevel": [],
    }
    v2_turn_ms: List[float] = []

    for g_idx in range(num_games):
        seed = base_seed + g_idx * 113
        rot = g_idx % 8
        seat_to_group: Dict[int, str] = {}
        bots: Dict[int, LobbyActor] = {}

        for logical_idx in range(8):
            seat = (logical_idx + rot) % 8
            s_seed = seed + logical_idx * 17
            if logical_idx in (0, 1):
                seat_to_group[seat] = "SearchV2"
                bots[seat] = ShopEpochBeamBot(seed=s_seed, legacy_mode=False)
            elif logical_idx in (2, 3):
                seat_to_group[seat] = "SearchV1_Old"
                bots[seat] = ShopEpochBeamBot(seed=s_seed, legacy_mode=True)
            elif logical_idx in (4, 5):
                seat_to_group[seat] = "Tempo"
                bots[seat] = TempoBot(seed=s_seed)
            else:
                seat_to_group[seat] = "GreedyLevel"
                bots[seat] = GreedyLevelBot(seed=s_seed)

        names = [f"{seat_to_group[p]}_p{p}" for p in range(8)]
        lobby = seaglass.LobbyGame(players=8, duos=False, seed=seed, names=names)

        while not lobby.done:
            lobby.begin_phase()
            is_turn = lobby.phase == "turn"
            for p in list(lobby.active_players()):
                is_v2 = seat_to_group[p] == "SearchV2"
                t0 = time.perf_counter() if (is_v2 and is_turn) else 0.0
                steps = 0
                while not lobby.is_done(p) and steps < MAX_ACTIONS_PER_TURN:
                    act = bots[p].act_in_lobby(lobby, p)
                    if not lobby.is_legal(p, act):
                        lobby.force_end(p)
                        break
                    note = getattr(bots[p], "last_note", None)
                    lobby.step(p, act, note=note)
                    steps += 1
                if not lobby.is_done(p):
                    lobby.force_end(p)
                if is_v2 and is_turn:
                    v2_turn_ms.append((time.perf_counter() - t0) * 1000.0)
            lobby.end_phase()

        places = lobby.placements
        v2_places = [places[s] for s in range(8) if seat_to_group[s] == "SearchV2"]
        v1_places = [places[s] for s in range(8) if seat_to_group[s] == "SearchV1_Old"]

        if save_replay_dir:
            os.makedirs(save_replay_dir, exist_ok=True)
            best_p = min(x for x in v2_places if x is not None)
            out_path = os.path.join(
                save_replay_dir,
                f"v2_head2head_g{g_idx + 1}_seed{seed}_best{best_p}.replay.pb",
            )
            lobby.set_meta("bot", "ShopEpochBeamBot_V2_NeuralPhysics")
            lobby.save(out_path)
            print(f"  -> saved {out_path}")

        for seat, place in enumerate(places):
            if place is not None:
                placements_by_group[seat_to_group[seat]].append(place)
        print(
            f"[H2H Game {g_idx + 1}/{num_games} | turns={lobby.turn}] "
            f"SearchV2={v2_places} vs SearchV1_Old={v1_places} | all={places}"
        )

    print("\n=== Head-to-Head 8-Player League Summary ===")
    for grp, pls in placements_by_group.items():
        avg_p = sum(pls) / max(1, len(pls))
        top4 = 100.0 * sum(1 for x in pls if x <= 4) / max(1, len(pls))
        win1 = 100.0 * sum(1 for x in pls if x == 1) / max(1, len(pls))
        print(
            f"  {grp:<18} | n={len(pls):>2} | Avg Place: {avg_p:.2f} "
            f"| Top-4: {top4:5.1f}% | 1st: {win1:5.1f}%"
        )
    if v2_turn_ms:
        mean_ms = sum(v2_turn_ms) / len(v2_turn_ms)
        p95_ms = sorted(v2_turn_ms)[int(0.95 * (len(v2_turn_ms) - 1))]
        print(f"\nSearchV2 Turn Latency: mean={mean_ms:.1f} ms/turn, p95={p95_ms:.1f} ms/turn")


def main() -> None:
    parser = argparse.ArgumentParser(description="Seaglass Python Search Bots Arena")
    parser.add_argument("--mode", choices=["solo", "duos", "both", "head2head"], default="both")
    parser.add_argument("--bot", choices=["beam", "mcts"], default="beam")
    parser.add_argument("--games", type=int, default=6)
    parser.add_argument("--seed", type=int, default=1000)
    parser.add_argument("--replay-dir", type=str, default=None)
    args = parser.parse_args()

    if args.mode == "head2head":
        run_head2head_benchmark(
            num_games=args.games,
            base_seed=args.seed,
            save_replay_dir=args.replay_dir,
        )
        return

    if args.mode in ("solo", "both"):
        run_solo_benchmark(
            num_games=args.games,
            bot_kind=args.bot,
            base_seed=args.seed,
            save_replay_dir=args.replay_dir,
        )
    if args.mode in ("duos", "both"):
        run_duos_benchmark(
            num_games=max(2, args.games // 2),
            base_seed=args.seed + 4000,
            save_replay_dir=args.replay_dir,
        )


if __name__ == "__main__":
    main()
