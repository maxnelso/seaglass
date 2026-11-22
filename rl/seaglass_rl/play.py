"""Play full lobbies (`seaglass.LobbyGame`, `engine/docs/lobby_game.md`) with a mix of controllers,
in lockstep, and collect training data.

The engine runs the game: hero offers, the shared card pool, pairings, ghosts, combats, knockouts
and placements. This module only decides who acts and what they do.

A controller drives one unit of a lobby (a Solo player, or both players of a Duos team). It is named
by a string, so that game specs can be sent to worker processes:
- `"learner"`: the model being trained. Its decisions are recorded as trajectories.
- `"heuristic"`, `"random"`, `"9kgamerbot"`: `bots.HeuristicBot`, `bots.RandomBot`,
  `gamerbot.NineKGamerBot`. Recording a bot (`record`)
  gives its decisions for behavior cloning (`pretrain.py`), with log-probs and values of 0.
- any other name: a model passed in `models` (a frozen past checkpoint, an agent being evaluated).

Every game is played phase by phase. Within a phase, each step lets every seat still to act take one
action. The model seats of all games are batched into one forward pass per model.

Rewards (for the recorded seats):
- **Placement**, when the seat's unit is placed (knocked out, or the game ends):
  `placement_reward(place, units)`, from +1 for first to -1 for last.
- **Optional shaping**: potential-based, with potential
  `shaping * log1p(board power) + health_shaping * (Health + Armor) / 40` (`potential`). Being
  potential-based, it changes no optimal policy and only moves credit earlier: the board's strength
  while building it, the damage when it is taken.
- **Discounting per turn**: `gamma` applies when a turn ends (`turn_end`), not at every action.

The model and recorded-bot seats' observations and masks come from the Rust `seaglass.Encoder` when
the extension has one (`PlayOptions.rust_encoder`, on by default); `features.py` and `actions.py` are
the reference it matches bit for bit.

Turn stats (`PlayOptions.turn_stats`, Solo only) record every living player's board, Tier, Gold and
actions as each turn's shopping ends. With `PlayOptions.probe`, the boards of `GameSpec.probe_units`
are also scored against a library of reference boards (`probe.py`).
"""

from __future__ import annotations

import math
import multiprocessing as mp
import os
import random
from collections import OrderedDict
from dataclasses import dataclass, field
from typing import TYPE_CHECKING, Any, Callable

import numpy as np
import seaglass

from .actions import (
    ACTION_KIND_IDS,
    ACTION_KINDS,
    ACTION_SPACE_SIZE,
    ACTION_TO_INDEX,
    END_TURN_INDEX,
    action_from_index,
    action_mask,
)
from .bots import HeuristicBot, RandomBot
from .gamerbot import NineKGamerBot
from .env import compute_board_power
from . import features, packing
from .features import NUM_SLOTS, PLAYER_DIM, SLOT_NUM_DIM, encode_observation, fast_encode, fast_mask
from .model import BattlegroundsMLP

if TYPE_CHECKING:
    from .positioning import PositionSpec
    from .search import SearchSpec
    from .probe import ProbeSpec

LEARNER = "learner"
BOTS = ("heuristic", "random", "9kgamerbot", "9kgamerbot-tempo", "apex", "sovereign")
# The Reposition actions: allowed only in the positioning step (`PlayOptions.positioning_phase`).
REPOSITION = ACTION_KIND_IDS == ACTION_KINDS.index("reposition")
# The Toggle Freeze action: with the positioning step, allowed only there, once a turn.
FREEZE = ACTION_KIND_IDS == ACTION_KINDS.index("toggle_freeze")


def potential(obs: Any, board: float = 1.0, health: float = 0.0) -> float:
    """The shaping potential of a state: `board` × `log1p` of the board power
    (`env.compute_board_power`; `log1p` so that late-game boards with huge stats don't swamp the
    placement reward), plus `health` × (Health + Armor) / 40 (a Duos team's shared pool)."""
    value = 0.0
    if board:
        value += board * math.log1p(max(0.0, compute_board_power(obs)))
    if health:
        value += health * (obs.health + obs.armor) / 40.0
    return value


def placement_reward(place: int, units: int) -> float:
    """+1 for first place, -1 for last, linear in between."""
    if units <= 1:
        return 0.0
    return 1.0 - 2.0 * (place - 1) / (units - 1)


@dataclass
class LobbyConfig:
    """The rules of the lobbies to play: `seaglass.LobbyGame`'s settings, minus the seed."""

    duos: bool = False
    # Solo: 2..8 players. Duos: 4, 6 or 8 (2..4 teams).
    players: int = 8
    max_turns: int = 30
    heroes: bool = True
    hero_offers: int = 2
    trinkets: bool = True
    dark_gifts: bool = True
    catalog: str | None = None
    first_fighter_switch_chance: float = 0.75
    avoid_recent: int = 3
    # The game's damage caps (`engine/docs/lobby_game.md` §5).
    damage_caps: bool = True
    # The simulated turn clock (`engine/docs/lobby_game.md` §2): each action costs `action_ms`, and a
    # player whose clock runs out has their turn ended for them.
    turn_timer: bool = True
    action_ms: int = 250
    # The minion types in each lobby, drawn at random (0: every type).
    tribes: int = 5

    def __post_init__(self) -> None:
        if self.duos and (self.players % 2 or not 4 <= self.players <= 8):
            raise ValueError(f"a Duos lobby has 4, 6 or 8 players, got {self.players}")
        if not self.duos and not 2 <= self.players <= 8:
            raise ValueError(f"a Solo lobby has 2..8 players, got {self.players}")

    @property
    def units(self) -> int:
        """The number of placed units: players in Solo, teams in Duos."""
        return self.players // 2 if self.duos else self.players

    def make(self, seed: int, meta: dict[str, str] | None = None) -> seaglass.LobbyGame:
        """A new lobby with these rules."""
        return seaglass.LobbyGame(
            players=self.players,
            duos=self.duos,
            seed=seed,
            max_turns=self.max_turns,
            heroes=self.heroes,
            hero_offers=self.hero_offers,
            trinkets=self.trinkets,
            dark_gifts=self.dark_gifts,
            catalog=self.catalog,
            first_fighter_switch_chance=self.first_fighter_switch_chance,
            avoid_recent=self.avoid_recent,
            damage_caps=self.damage_caps,
            turn_timer=self.turn_timer,
            action_ms=self.action_ms,
            tribes=self.tribes,
            meta=meta,
        )


class Seat:
    """One player of a `seaglass.LobbyGame`, with the `TavernGame` interface (`observe`,
    `legal_actions`, `is_legal`, `step`, `end_turn`) that the bots and `action_mask` use."""

    def __init__(self, game: seaglass.LobbyGame, player: int) -> None:
        self.game = game
        self.player = player

    def observe(self) -> Any:
        return self.game.observe(self.player)

    def legal_actions(self) -> list[seaglass.TavernAction]:
        return self.game.legal_actions(self.player)

    def is_legal(self, action: seaglass.TavernAction) -> bool:
        return self.game.is_legal(self.player, action)

    def step(self, action: seaglass.TavernAction, note: dict[str, Any] | None = None) -> bool:
        return self.game.step(self.player, action, note)

    def end_turn(self, note: dict[str, Any] | None = None) -> None:
        self.game.step(self.player, seaglass.TavernAction.end_turn(), note)


@dataclass
class PlayOptions:
    """How the controllers play."""

    # A cap on a model's decisions per phase, on top of the engine's turn timer. None: the timer only.
    # Set it when free actions can loop (e.g. without the positioning step, a model can Reposition back
    # and forth until its clock runs out).
    max_steps_per_turn: int | None = None
    # A model's turn has two parts: shopping (everything but Reposition and Toggle Freeze), then,
    # once it picks End Turn, an end-of-turn step ("prepping the board for battle"): only
    # Repositions (at most board size - 1 of them), one Toggle Freeze, and End Turn. Without it, a
    # model may reposition and freeze at any time (`allow_reposition`, `allow_freeze`).
    positioning_phase: bool = True
    allow_reposition: bool = True
    allow_freeze: bool = True
    prune_dominated: bool = False
    deterministic: bool = False
    gamma: float = 0.99
    shaping: float = 0.0
    # The Health potential's coefficient (`potential`).
    health_shaping: float = 0.0
    # Record the models' value and log-probability with their actions, and keep every game's
    # replay (`GameResult.replay`, for `match_replay`).
    keep_replays: bool = False
    # Collect `GameResult.turn_stats` for every player (Solo only).
    turn_stats: bool = False
    # With `turn_stats`: also score the end-of-turn board of each unit in `GameSpec.probe_units`
    # against the reference library (`probe.py`).
    probe: "ProbeSpec | None" = None
    # Order every model seat's board by search with the combat simulator just before its turn ends
    # (`positioning.py`; Solo only). The moves are counted as "auto_position", never recorded.
    auto_position: "PositionSpec | None" = None
    # Encode the observations and build the masks of the model and recorded-bot seats with the Rust
    # encoder (`features.rust_encoder`; bit-identical to the Python reference, about 10x faster), when
    # the installed extension has one. False: the Python `encode_observation` and `action_mask`.
    rust_encoder: bool = True
    # Return the trajectories packed (`Trajectory.arrays(pack=True)`, `packing.py`): far smaller to
    # send between processes and to keep. The trainer and pretrain always pack.
    pack: bool = False
    # Policy-guided search inside the turn (`search.py`; Solo only): a model seat whose controller is
    # in `search_controllers` takes the search's action at its shopping decisions (the end-of-turn
    # step and the hero phase are played by the policy). With `search=None` nothing changes.
    search: "SearchSpec | None" = None
    search_controllers: tuple[str, ...] = ()
    # Auxiliary targets (Solo only): every recorded seat's end-of-turn board is scored with the probe
    # (`probe.py`), and each recorded decision on turn t gets the probe scores of turns t + k for k in
    # `aux_horizons` as `aux_target` / `aux_mask` (`Trajectory.arrays`). The fights are the added cost.
    aux_probe: "ProbeSpec | None" = None
    aux_horizons: tuple[int, ...] = (0, 2, 4)


@dataclass
class GameSpec:
    """One game to play: its seed and each unit's controller."""

    seed: int
    controllers: list[str]
    # The units whose boards are probed (`PlayOptions.probe`).
    probe_units: tuple[int, ...] = ()


@dataclass
class TurnStat:
    """One player's turn, as it stood when the shopping ended (before the combats)."""

    turn: int
    tier: int  # Tavern Tier
    gold_left: int  # Gold unspent
    health: int  # Health + Armor
    board: int  # minions on the board
    actions: dict[str, int]  # actions taken this turn, by kind
    probe: float | None = None  # `probe.score_board` of the board, for probed units


@dataclass
class Trajectory:
    """One recorded seat's decisions in one game, in order."""

    game: int
    player: int
    player_vec: list[np.ndarray] = field(default_factory=list)
    slot_card_ids: list[np.ndarray] = field(default_factory=list)
    slot_num_feats: list[np.ndarray] = field(default_factory=list)
    # With the Rust encoder and `pack`, the nonzeros of each decision's features (uint16 columns,
    # float16 values, zeros after the cast dropped) stand in for `slot_num_feats`.
    num_idx: list[np.ndarray] = field(default_factory=list)
    num_val: list[np.ndarray] = field(default_factory=list)
    action_mask: list[np.ndarray] = field(default_factory=list)
    actions: list[int] = field(default_factory=list)
    log_probs: list[float] = field(default_factory=list)
    values: list[float] = field(default_factory=list)
    rewards: list[float] = field(default_factory=list)
    turn_end: list[bool] = field(default_factory=list)
    done: bool = False
    # The unit's final place (1 is first); 0 while unknown.
    place: int = 0
    # The shaping potential of each decision's state.
    potentials: list[float] = field(default_factory=list)
    # With search: each decision's improved policy (`SearchResult.policy_target`), zeros where the
    # decision was not searched. None without search.
    search_policy: list[np.ndarray] | None = None
    # With `PlayOptions.aux_probe`: each decision's turn (0 in the hero phase), the seat's probe score
    # by turn (a turn it was alive at the end of and the library covers), and the horizons.
    decision_turn: list[int] | None = None
    probe_by_turn: dict[int, float] = field(default_factory=dict)
    aux_horizons: tuple[int, ...] = ()

    def aux_arrays(self) -> tuple[np.ndarray, np.ndarray]:
        """`(target float32 [T, H], mask bool [T, H])`: decision `i` on turn `t` has target
        `probe(t + k)` for horizon `k`, masked out where that probe doesn't exist (the seat was
        knocked out, the game ended, or the library has no boards then) and for the hero phase."""
        turns = self.decision_turn or []
        assert len(turns) == len(self.actions)
        target = np.zeros((len(turns), len(self.aux_horizons)), dtype=np.float32)
        mask = np.zeros(target.shape, dtype=np.bool_)
        for i, t in enumerate(turns):
            if t <= 0:
                continue
            for j, k in enumerate(self.aux_horizons):
                v = self.probe_by_turn.get(t + k)
                if v is not None:
                    target[i, j] = v
                    mask[i, j] = True
        return target, mask

    def __len__(self) -> int:
        return len(self.actions)

    def arrays(self, pack: bool = False) -> dict[str, np.ndarray]:
        """The trajectory as arrays (compact dtypes, for sending between processes).

        With `pack`, `slot_num_feats` becomes CSR rows (`num_ptr`, `num_idx`, `num_val`), `action_mask`
        becomes `mask_bits` (`np.packbits`), `slot_card_ids` is uint16, and `packed` is True
        (`packing.py`)."""
        out = {
            "player_vec": np.asarray(self.player_vec, dtype=np.float32).reshape(-1, PLAYER_DIM),
            "actions": np.asarray(self.actions, dtype=np.int64),
            "log_probs": np.asarray(self.log_probs, dtype=np.float32),
            "values": np.asarray(self.values, dtype=np.float32),
            "rewards": np.asarray(self.rewards, dtype=np.float32),
            "turn_end": np.asarray(self.turn_end, dtype=np.bool_),
            "done": np.asarray(self.done),
            "place": np.asarray(self.place, dtype=np.int64),
        }
        if self.decision_turn is not None:
            out["aux_target"], out["aux_mask"] = self.aux_arrays()
        if self.search_policy is not None:
            out["search_policy"] = np.asarray(self.search_policy, dtype=np.float16).reshape(-1, ACTION_SPACE_SIZE)
        sparse = bool(self.num_idx)
        feats = None
        if not sparse:
            feats = np.asarray(self.slot_num_feats, dtype=np.float16).reshape(-1, NUM_SLOTS, SLOT_NUM_DIM)
        mask = np.asarray(self.action_mask, dtype=np.bool_).reshape(-1, ACTION_SPACE_SIZE)
        if pack:
            assert features.VOCAB_SIZE < 65536
            out["slot_card_ids"] = np.asarray(self.slot_card_ids, dtype=np.uint16).reshape(-1, NUM_SLOTS)
            if sparse:
                ptr = np.zeros(len(self.num_idx) + 1, dtype=np.int64)
                np.cumsum([len(i) for i in self.num_idx], out=ptr[1:])
                out["num_ptr"] = ptr
                out["num_idx"] = np.concatenate(self.num_idx)
                out["num_val"] = np.concatenate(self.num_val)
            else:
                out["num_ptr"], out["num_idx"], out["num_val"] = packing.pack_rows(feats)
            out["mask_bits"] = packing.pack_mask(mask)
            out["packed"] = np.asarray(True)
        else:
            assert not sparse, "sparse trajectories can only be packed"
            out["slot_card_ids"] = np.asarray(self.slot_card_ids, dtype=np.int32).reshape(-1, NUM_SLOTS)
            out["slot_num_feats"] = feats
            out["action_mask"] = mask
        return out


@dataclass
class GameResult:
    """How one game went."""

    seed: int
    controllers: list[str]
    # Each unit's place (1 is first).
    placements: list[int]
    turns: int
    trajectories: list[dict[str, np.ndarray]]
    # The game's replay (protobuf bytes), with `PlayOptions.keep_replays`.
    replay: bytes | None = None
    # Each player's turns (`PlayOptions.turn_stats`; empty without it): one entry per turn in which
    # the player's unit was alive when the shopping ended.
    turn_stats: list[list[TurnStat]] = field(default_factory=list)

    def save_replay(self, path: str | os.PathLike[str]) -> None:
        if self.replay is None:
            raise ValueError("the game was played without `PlayOptions.keep_replays`")
        os.makedirs(os.path.dirname(os.fspath(path)) or ".", exist_ok=True)
        with open(path, "wb") as f:
            f.write(self.replay)


class _Seat:
    """Bookkeeping for one seat during play."""

    def __init__(self, controller: str, bot: Any | None, traj: Trajectory | None) -> None:
        self.controller = controller
        self.bot = bot
        self.traj = traj
        self.steps = 0
        self.context = None
        # In the positioning step, the Repositions left in it, and whether it froze (or unfroze)
        # the shop this turn.
        self.arranging = False
        self.moves_left = 0
        self.froze = False
        # The actions taken this phase, by kind.
        self.kinds: dict[str, int] = {}


def _bot(controller: str, seed: int) -> Any | None:
    if controller == "heuristic":
        return HeuristicBot()
    if controller == "random":
        return RandomBot(seed=seed)
    if controller == "9kgamerbot":
        return NineKGamerBot()
    if controller == "9kgamerbot-tempo":
        return NineKGamerBot(tempo_levels=True)
    if controller == "apex":
        from seaglass_bots import ApexBot

        return ApexBot(seed=seed)
    if controller == "sovereign":
        from seaglass_bots import SovereignBot

        return SovereignBot(seed=seed)
    return None


def play_games(
    config: LobbyConfig,
    specs: list[GameSpec],
    models: dict[str, BattlegroundsMLP],
    options: PlayOptions | None = None,
    record: tuple[str, ...] = (LEARNER,),
    *,
    on_turn_end: Callable[[int, int, seaglass.LobbyGame, Any], None] | None = None,
) -> list[GameResult]:
    """Play `specs` to the end, in lockstep. `models` maps every model controller's name to its
    model. The seats of the `record` controllers are returned as trajectories.

    `on_turn_end(g, p, game, obs)` is called for every living player of game `g` when a turn's
    shopping ends, before the combats. It is for in-process use (the probe library builder).
    """
    opts = options or PlayOptions()
    if (opts.turn_stats or opts.probe is not None or opts.aux_probe is not None) and config.duos:
        raise ValueError("turn stats, the probe and the auxiliary probe targets are Solo-only")
    searching = opts.search is not None and bool(opts.search_controllers)
    if searching and config.duos:
        raise ValueError("search is Solo-only")
    if searching:
        from . import search
    if opts.auto_position is not None and config.duos:
        raise ValueError("auto positioning is Solo-only")
    if opts.auto_position is not None:
        from . import positioning
    library = None
    if opts.probe is not None:
        from . import probe

        library = probe.library_for(opts.probe, config)
    aux_library = None
    if opts.aux_probe is not None:
        from . import probe

        aux_library = probe.library_for(opts.aux_probe, config)
    stats: list[list[list[TurnStat]]] = [[[] for _ in range(config.players)] for _ in specs]
    games = [
        config.make(spec.seed, meta={"source": "seaglass_rl.play", "controllers": ",".join(spec.controllers)})
        for spec in specs
    ]
    seats: list[list[_Seat]] = []
    views: list[list[Seat]] = []
    for g, (game, spec) in enumerate(zip(games, specs)):
        if len(spec.controllers) != config.units:
            raise ValueError(f"game {g}: {len(spec.controllers)} controllers for {config.units} units")
        row = []
        for p in range(config.players):
            ctrl = spec.controllers[game.unit_of(p)]
            if ctrl not in BOTS and ctrl not in models:
                raise ValueError(f"no model for controller {ctrl!r}")
            traj = Trajectory(game=g, player=p) if ctrl in record else None
            if traj is not None and opts.aux_probe is not None:
                traj.decision_turn = []
                traj.aux_horizons = tuple(opts.aux_horizons)
            if traj is not None and searching and ctrl in opts.search_controllers:
                traj.search_policy = []
            row.append(_Seat(ctrl, _bot(ctrl, spec.seed * 16 + p), traj))
        seats.append(row)
        views.append([Seat(game, p) for p in range(config.players)])

    rust = features.rust_encoder() if opts.rust_encoder else None
    sparse = rust is not None and opts.pack
    # The shaping potential is the only thing left that reads a Python observation of a model seat.
    needs_obs = bool(opts.shaping or opts.health_shaping)

    def base_mask(seat: Seat, obs: Any, reposition: bool, freeze: bool) -> np.ndarray:
        if rust is not None:
            return fast_mask(rust, seat.game, seat.player, reposition, freeze, opts.prune_dominated)
        return action_mask(
            seat,
            allow_reposition=reposition,
            allow_freeze=freeze,
            prune_dominated=opts.prune_dominated,
            obs=obs,
        )

    def mask_for(seat: Seat, obs: Any, st: _Seat) -> np.ndarray:
        if not opts.positioning_phase:
            return base_mask(seat, obs, opts.allow_reposition, opts.allow_freeze)
        mask = base_mask(seat, obs, st.arranging, opts.allow_freeze and st.arranging and not st.froze)
        if st.arranging:
            # The end-of-turn step: Repositions (while any are left), one Toggle Freeze, End Turn.
            if st.moves_left <= 0:
                mask &= ~REPOSITION
            mask &= REPOSITION | FREEZE | (np.arange(ACTION_SPACE_SIZE) == END_TURN_INDEX)
        return mask

    def position(g: int, p: int) -> None:
        """Order the model seat's board by search, just before its turn ends."""
        pos = opts.auto_position
        game, st = games[g], seats[g][p]
        if pos is None or st.bot is not None or game.is_done(p):
            return
        obs = views[g][p].observe()
        board = list(obs.board)
        if len(board) < 2:
            return
        opponents = positioning.opponent_boards(game, p, config, pos)
        if not opponents:
            return
        seed = specs[g].seed * 7_919 + p * 101 + game.turn
        perm, _ = positioning.best_order(board, opponents, obs.tavern_tier, seed, pos)
        for f, t in positioning.moves_to(perm):
            idx = ACTION_TO_INDEX[("reposition", f, t)]
            if not game.is_legal(p, action_from_index(idx)):
                break
            _apply(game, p, st, idx, {"auto": "position"} if opts.keep_replays else None, kind="auto_position")

    def model_step(g: int, p: int, idx: int, note: dict[str, Any] | None, board: int) -> None:
        """Take a model's action, with the end-of-turn step ("prepping the board for battle"): its
        first End Turn opens the step instead of ending the turn, if there is anything to do in it
        (2+ minions to reorder, or the shop to freeze). In the step, it may make up to (board - 1)
        Repositions and one Toggle Freeze; the step ends with End Turn, or by itself once there is
        nothing left to do."""
        st = seats[g][p]
        can_freeze = opts.allow_freeze and not st.froze
        if opts.positioning_phase and idx == END_TURN_INDEX and not st.arranging and (board >= 2 or can_freeze):
            st.arranging = True
            st.moves_left = board - 1 if opts.allow_reposition else 0
            st.steps += 1
            return
        if idx == END_TURN_INDEX:
            position(g, p)
        _apply(games[g], p, st, idx, note)
        if st.arranging and idx != END_TURN_INDEX:
            if FREEZE[idx]:
                st.froze = True
            else:
                st.moves_left -= 1
            if st.moves_left <= 0 and (st.froze or not opts.allow_freeze) and not games[g].is_done(p):
                position(g, p)
                _apply(games[g], p, st, END_TURN_INDEX, None)

    def close(traj: Trajectory, reward: float, place: int) -> None:
        """The seat's unit was placed: the terminal reward (and potential 0) end it."""
        traj.place = place
        if traj.done or not traj.actions:
            traj.done = True
            return
        traj.rewards[-1] += reward - traj.potentials[-1]
        traj.turn_end[-1] = True
        traj.done = True

    def force_end(g: int, p: int) -> None:
        games[g].force_end(p)
        traj = seats[g][p].traj
        if traj is not None and traj.actions:
            traj.turn_end[-1] = True

    while True:
        live = [g for g, game in enumerate(games) if not game.done]
        if not live:
            break
        for g in live:
            games[g].begin_phase()
            for st in seats[g]:
                st.steps = 0
                st.context = None
                st.arranging = False
                st.moves_left = 0
                st.froze = False
                st.kinds = {}
        step = 0
        while True:
            # Who acts now: every player still to act, unless they are over the step cap. In Duos,
            # one player per team at a time (alternating), since a player's action can change
            # their teammate's Tavern and so the mask the teammate would act on.
            todo: list[tuple[int, int]] = []
            for g in live:
                game = games[g]
                active = game.active_players()
                if config.duos and step % 2:
                    active = active[::-1]
                units: set[int] = set()
                for p in active:
                    st = seats[g][p]
                    cap = opts.max_steps_per_turn if st.bot is None else None
                    if cap is not None and st.steps >= cap:
                        force_end(g, p)
                        continue
                    unit = game.unit_of(p)
                    if unit in units:
                        continue
                    units.add(unit)
                    todo.append((g, p))
            if not todo:
                break
            step += 1
            # The bots act one by one; the models in one batch each.
            by_model: dict[str, list[tuple[int, int, tuple, np.ndarray, float, int]]] = {}
            for g, p in todo:
                game, st, seat = games[g], seats[g][p], views[g][p]
                if st.bot is not None and st.traj is None:
                    idx = st.bot.act(seat)
                    if not seat.is_legal(action_from_index(idx)):
                        idx = END_TURN_INDEX
                    _apply(game, p, st, idx, None)
                    continue
                if rust is not None:
                    obs = seat.observe() if needs_obs else None
                    nboard = rust.board_len(game, p)
                    enc = fast_encode(rust, game, p, st.arranging, game.phase == "turn", sparse=sparse and st.traj is not None)
                else:
                    obs = seat.observe()
                    if st.context is None and game.phase == "turn":
                        st.context = game.context(p)
                    nboard = len(obs.board)
                    enc = encode_observation(obs, st.context, arranging=st.arranging)
                mask = mask_for(seat, obs, st)
                if not mask.any():
                    force_end(g, p)
                    continue
                phi = potential(obs, opts.shaping, opts.health_shaping)
                if st.bot is not None:
                    # A recorded bot (for behavior cloning): its action, with no log-prob or value.
                    idx = st.bot.act(seat)
                    if not seat.is_legal(action_from_index(idx)):
                        idx = END_TURN_INDEX
                    mask[idx] = True
                    _record(st.traj, enc, mask, idx, 0.0, 0.0, phi, opts.gamma)
                    board = nboard
                    if (
                        idx == END_TURN_INDEX
                        and opts.positioning_phase
                        and game.phase == "turn"
                        and (board >= 2 or (opts.allow_freeze and not st.froze))
                    ):
                        # A model's End Turn would open the end-of-turn step here. Record the bot as
                        # ending the step too, so a clone has seen that state and leaves it (bots
                        # reposition and freeze while shopping instead).
                        st.arranging = True
                        st.moves_left = board - 1 if opts.allow_reposition else 0
                        if rust is not None:
                            enc = fast_encode(rust, game, p, True, game.phase == "turn", sparse=sparse and st.traj is not None)
                        else:
                            enc = encode_observation(obs, st.context, arranging=True)
                        mask = mask_for(seat, obs, st)
                        mask[idx] = True
                        _record(st.traj, enc, mask, idx, 0.0, 0.0, phi, opts.gamma)
                    _apply(game, p, st, idx, None)
                    continue
                if (
                    searching
                    and st.controller in opts.search_controllers
                    and game.phase == "turn"
                    and not st.arranging
                    and int(mask.sum()) >= 2
                ):
                    dseed = search.decision_seed(specs[g].seed, p, game.turn, st.steps)
                    res = search.search_decision(
                        models[st.controller],
                        game,
                        p,
                        opts,
                        opts.search,
                        np.random.default_rng(search.decision_seed(opts.search.seed, dseed, 1)),
                        dseed,
                        root=(enc, mask),
                    )
                    if st.traj is not None:
                        _record(st.traj, enc, mask, res.action, res.log_prob, res.root_value, phi, opts.gamma)
                        st.traj.search_policy[-1] = res.policy_target
                    note = {"value": res.root_value, "logp": res.log_prob} if opts.keep_replays else None
                    model_step(g, p, res.action, note, nboard)
                    continue
                by_model.setdefault(st.controller, []).append((g, p, enc, mask, phi, nboard))
            for ctrl, rows in by_model.items():
                model = models[ctrl]
                pv = np.stack([r[2][0] for r in rows])
                ci = np.stack([r[2][1] for r in rows])
                nf = np.stack([r[2][2] for r in rows])
                mk = np.stack([r[3] for r in rows])
                acts, lps, vals = model.act_batch(pv, ci, nf, mk, deterministic=opts.deterministic)
                for i, (g, p, enc, mask, phi, board) in enumerate(rows):
                    st = seats[g][p]
                    if st.traj is not None:
                        _record(st.traj, enc, mask, int(acts[i]), float(lps[i]), float(vals[i]), phi, opts.gamma)
                    note = {"value": float(vals[i]), "logp": float(lps[i])} if opts.keep_replays else None
                    model_step(g, p, int(acts[i]), note, board)
        for g in live:
            game = games[g]
            if opts.aux_probe is not None:
                # Stamp this phase's decisions with their turn (0 in the hero phase).
                for st in seats[g]:
                    if st.traj is not None:
                        st.traj.decision_turn.extend(
                            [game.turn if game.phase == "turn" else 0] * (len(st.traj.actions) - len(st.traj.decision_turn))
                        )
            if (opts.turn_stats or opts.aux_probe is not None or on_turn_end is not None) and game.phase == "turn":
                alive = game.alive
                for p in range(config.players):
                    unit = game.unit_of(p)
                    if not alive[unit]:
                        continue
                    obs = game.observe(p)
                    if on_turn_end is not None:
                        on_turn_end(g, p, game, obs)
                    # One fight set per (seat, turn): the turn stat and the aux target share it.
                    scores: dict[ProbeSpec, float | None] = {}

                    def probe_of(spec: "ProbeSpec", lib: Any) -> float | None:
                        if spec not in scores:
                            scores[spec] = probe.score_board(
                                lib,
                                obs.board,
                                obs.tavern_tier,
                                game.turn,
                                seed=specs[g].seed * 1_000_003 + p * 1_009 + game.turn,
                                sims=spec.sims,
                            )
                        return scores[spec]

                    if opts.turn_stats:
                        stat = TurnStat(
                            turn=game.turn,
                            tier=obs.tavern_tier,
                            gold_left=obs.gold,
                            health=obs.health + obs.armor,
                            board=len(obs.board),
                            actions=dict(seats[g][p].kinds),
                        )
                        if opts.probe is not None and unit in specs[g].probe_units:
                            stat.probe = probe_of(opts.probe, library)
                        stats[g][p].append(stat)
                    traj = seats[g][p].traj
                    if opts.aux_probe is not None and traj is not None:
                        score = probe_of(opts.aux_probe, aux_library)
                        if score is not None:
                            traj.probe_by_turn[game.turn] = score
            for unit in game.end_phase():
                place = game.placements[unit]
                assert place is not None
                for p in game.unit_players(unit):
                    traj = seats[g][p].traj
                    if traj is not None:
                        close(traj, placement_reward(place, config.units), place)

    results = []
    for g, (game, spec) in enumerate(zip(games, specs)):
        trajs = [s.traj.arrays(pack=opts.pack) for s in seats[g] if s.traj is not None and s.traj.actions]
        results.append(
            GameResult(
                seed=spec.seed,
                controllers=list(spec.controllers),
                placements=[int(p) for p in game.placements],
                turns=game.turn,
                trajectories=trajs,
                replay=bytes(game.replay_bytes()) if opts.keep_replays else None,
                turn_stats=stats[g] if opts.turn_stats else [],
            )
        )
    return results


_NO_TARGET = np.zeros(ACTION_SPACE_SIZE, dtype=np.float32)


def _record(
    traj: Trajectory,
    enc: tuple,
    mask: np.ndarray,
    action: int,
    log_prob: float,
    value: float,
    phi: float,
    gamma: float,
) -> None:
    """Append a decision to `traj`, first giving the previous decision its shaping, now that its
    next state's potential `phi` is known."""
    if traj.actions:
        disc = gamma if traj.turn_end[-1] else 1.0
        traj.rewards[-1] += disc * phi - traj.potentials[-1]
    traj.player_vec.append(enc[0])
    traj.slot_card_ids.append(enc[1])
    if len(enc) == 5:
        # Sparse: keep only the nonzeros, as `packing.pack_rows` would (it drops entries whose
        # float16 is zero, -0.0 included).
        val = enc[4].astype(np.float16)
        keep = (val.view(np.uint16) << np.uint16(1)) != 0
        if keep.all():
            traj.num_idx.append(enc[3])
            traj.num_val.append(val)
        else:
            traj.num_idx.append(enc[3][keep])
            traj.num_val.append(val[keep])
    else:
        traj.slot_num_feats.append(enc[2])
    traj.action_mask.append(mask)
    traj.actions.append(action)
    traj.log_probs.append(log_prob)
    traj.values.append(value)
    traj.rewards.append(0.0)
    traj.turn_end.append(False)
    traj.potentials.append(phi)
    if traj.search_policy is not None:
        traj.search_policy.append(_NO_TARGET)


def _apply(
    game: seaglass.LobbyGame,
    p: int,
    st: _Seat,
    idx: int,
    note: dict[str, Any] | None,
    kind: str | None = None,
) -> None:
    """Take action `idx` for player `p`; mark the end of their turn on their trajectory. `kind`
    overrides the action's kind in `st.kinds`."""
    done = game.step(p, action_from_index(idx), note)
    st.steps += 1
    kind = kind or ACTION_KINDS[ACTION_KIND_IDS[idx]]
    st.kinds[kind] = st.kinds.get(kind, 0) + 1
    if done and st.traj is not None and st.traj.actions:
        st.traj.turn_end[-1] = True


# -- Worker processes ---------------------------------------------------------------------------

# Checkpoint models loaded in this worker, least recently used first; capped so a long run with
# hundreds of snapshots doesn't grow a worker's memory without bound.
_MODEL_CACHE: OrderedDict[str, BattlegroundsMLP] = OrderedDict()
_MODEL_CACHE_SIZE = 32
_THREAD_VARS = ("OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS")


def _init_worker() -> None:
    import torch

    torch.set_num_threads(1)


def _model_from(name: str, source: Any, seed: int) -> BattlegroundsMLP:
    """A model from a checkpoint path (cached) or from `(config, state_dict)`."""
    from . import checkpoint

    if isinstance(source, str):
        model = _MODEL_CACHE.get(source)
        if model is None:
            model, _ = checkpoint.load(source)
            _MODEL_CACHE[source] = model
            while len(_MODEL_CACHE) > _MODEL_CACHE_SIZE:
                _MODEL_CACHE.popitem(last=False)
        else:
            _MODEL_CACHE.move_to_end(source)
    else:
        config, state = source
        model = checkpoint.model_from_config(config)
        model.load_state_dict(state)
        model.eval()
    model._np_rng = np.random.default_rng(seed)
    return model


def _play_chunk(job: tuple) -> list[GameResult]:
    config, specs, sources, options, record, seed = job
    models = {name: _model_from(name, src, seed + i) for i, (name, src) in enumerate(sources.items())}
    return play_games(config, specs, models, options, record)


class Player:
    """Plays games across worker processes (or in this one, with `workers=0`).

    `sources` map each model controller's name to a checkpoint path or to a
    `(model.config, model.state_dict())` pair.
    """

    def __init__(self, workers: int = 0) -> None:
        self.workers = workers
        self.pool = None
        if workers > 0:
            # One BLAS/OpenMP thread per worker: the workers' small matrix products would otherwise
            # oversubscribe the cores many times over. A spawned worker reads these before it
            # loads numpy and torch (a forked one would inherit the parent's thread pools).
            saved = {k: os.environ.get(k) for k in _THREAD_VARS}
            os.environ.update({k: "1" for k in _THREAD_VARS})
            try:
                self.pool = mp.get_context("spawn").Pool(workers, initializer=_init_worker)
            finally:
                for k, v in saved.items():
                    if v is None:
                        os.environ.pop(k, None)
                    else:
                        os.environ[k] = v

    def play(
        self,
        config: LobbyConfig,
        specs: list[GameSpec],
        sources: dict[str, Any],
        options: PlayOptions | None = None,
        record: tuple[str, ...] = (LEARNER,),
        seed: int = 0,
    ) -> list[GameResult]:
        options = options or PlayOptions()
        def used_by(chunk: list[GameSpec]) -> dict[str, Any]:
            # Only the models this chunk's games use (a worker loads them per chunk).
            names = {name for spec in chunk for name in spec.controllers}
            return {k: v for k, v in sources.items() if k in names}

        if self.pool is None:
            return _play_chunk((config, specs, used_by(specs), options, record, seed))
        n = max(1, min(self.workers, len(specs)))
        chunks = [specs[i::n] for i in range(n)]
        jobs = []
        for i, c in enumerate(chunks):
            jobs.append((config, c, used_by(c), options, record, seed + 7919 * i))
        results: list[GameResult] = []
        for part in self.pool.map(_play_chunk, jobs):
            results.extend(part)
        return results

    def close(self) -> None:
        if self.pool is not None:
            self.pool.close()
            self.pool.join()
            self.pool = None


def random_seats(units: int, agents: int, rng: random.Random) -> list[int]:
    """`agents` distinct units, at random."""
    return rng.sample(range(units), agents)
