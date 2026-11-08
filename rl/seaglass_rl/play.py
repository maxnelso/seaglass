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
"""

from __future__ import annotations

import math
import multiprocessing as mp
import os
import random
from dataclasses import dataclass, field
from typing import Any

import numpy as np
import seaglass

from .actions import (
    ACTION_KIND_IDS,
    ACTION_KINDS,
    ACTION_SPACE_SIZE,
    END_TURN_INDEX,
    action_from_index,
    action_mask,
)
from .bots import HeuristicBot, RandomBot
from .gamerbot import NineKGamerBot
from .env import compute_board_power
from .features import NUM_SLOTS, PLAYER_DIM, SLOT_NUM_DIM, encode_observation
from .model import BattlegroundsMLP

LEARNER = "learner"
BOTS = ("heuristic", "random", "9kgamerbot", "9kgamerbot-tempo")
# A bot's turn may take more steps than a model's (`PlayOptions.max_steps_per_turn`): 9kgamerbot
# rolls, buys and sells many times a turn.
BOT_MAX_STEPS_PER_TURN = 100
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

    max_steps_per_turn: int = 30
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


@dataclass
class GameSpec:
    """One game to play: its seed and each unit's controller."""

    seed: int
    controllers: list[str]


@dataclass
class Trajectory:
    """One recorded seat's decisions in one game, in order."""

    game: int
    player: int
    player_vec: list[np.ndarray] = field(default_factory=list)
    slot_card_ids: list[np.ndarray] = field(default_factory=list)
    slot_num_feats: list[np.ndarray] = field(default_factory=list)
    action_mask: list[np.ndarray] = field(default_factory=list)
    actions: list[int] = field(default_factory=list)
    log_probs: list[float] = field(default_factory=list)
    values: list[float] = field(default_factory=list)
    rewards: list[float] = field(default_factory=list)
    turn_end: list[bool] = field(default_factory=list)
    done: bool = False
    # The shaping potential of each decision's state.
    potentials: list[float] = field(default_factory=list)

    def __len__(self) -> int:
        return len(self.actions)

    def arrays(self) -> dict[str, np.ndarray]:
        """The trajectory as arrays (compact dtypes, for sending between processes)."""
        return {
            "player_vec": np.asarray(self.player_vec, dtype=np.float32).reshape(-1, PLAYER_DIM),
            "slot_card_ids": np.asarray(self.slot_card_ids, dtype=np.int32).reshape(-1, NUM_SLOTS),
            "slot_num_feats": np.asarray(self.slot_num_feats, dtype=np.float16).reshape(
                -1, NUM_SLOTS, SLOT_NUM_DIM
            ),
            "action_mask": np.asarray(self.action_mask, dtype=np.bool_).reshape(-1, ACTION_SPACE_SIZE),
            "actions": np.asarray(self.actions, dtype=np.int64),
            "log_probs": np.asarray(self.log_probs, dtype=np.float32),
            "values": np.asarray(self.values, dtype=np.float32),
            "rewards": np.asarray(self.rewards, dtype=np.float32),
            "turn_end": np.asarray(self.turn_end, dtype=np.bool_),
            "done": np.asarray(self.done),
        }


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


def _bot(controller: str, seed: int) -> Any | None:
    if controller == "heuristic":
        return HeuristicBot()
    if controller == "random":
        return RandomBot(seed=seed)
    if controller == "9kgamerbot":
        return NineKGamerBot()
    if controller == "9kgamerbot-tempo":
        return NineKGamerBot(tempo_levels=True)
    return None


def play_games(
    config: LobbyConfig,
    specs: list[GameSpec],
    models: dict[str, BattlegroundsMLP],
    options: PlayOptions | None = None,
    record: tuple[str, ...] = (LEARNER,),
) -> list[GameResult]:
    """Play `specs` to the end, in lockstep. `models` maps every model controller's name to its
    model. The seats of the `record` controllers are returned as trajectories."""
    opts = options or PlayOptions()
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
            row.append(_Seat(ctrl, _bot(ctrl, spec.seed * 16 + p), traj))
        seats.append(row)
        views.append([Seat(game, p) for p in range(config.players)])

    def mask_for(seat: Seat, obs: Any, st: _Seat) -> np.ndarray:
        if not opts.positioning_phase:
            return action_mask(
                seat,
                allow_reposition=opts.allow_reposition,
                allow_freeze=opts.allow_freeze,
                prune_dominated=opts.prune_dominated,
                obs=obs,
            )
        mask = action_mask(
            seat,
            allow_reposition=st.arranging,
            allow_freeze=opts.allow_freeze and st.arranging and not st.froze,
            prune_dominated=opts.prune_dominated,
            obs=obs,
        )
        if st.arranging:
            # The end-of-turn step: Repositions (while any are left), one Toggle Freeze, End Turn.
            if st.moves_left <= 0:
                mask &= ~REPOSITION
            mask &= REPOSITION | FREEZE | (np.arange(ACTION_SPACE_SIZE) == END_TURN_INDEX)
        return mask

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
        _apply(games[g], p, st, idx, note)
        if st.arranging and idx != END_TURN_INDEX:
            if FREEZE[idx]:
                st.froze = True
            else:
                st.moves_left -= 1
            if st.moves_left <= 0 and (st.froze or not opts.allow_freeze) and not games[g].is_done(p):
                _apply(games[g], p, st, END_TURN_INDEX, None)

    def close(traj: Trajectory, reward: float) -> None:
        """The seat's unit was placed: the terminal reward (and potential 0) end it."""
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
                    cap = opts.max_steps_per_turn if st.bot is None else BOT_MAX_STEPS_PER_TURN
                    if st.steps >= cap:
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
                obs = seat.observe()
                if st.context is None and game.phase == "turn":
                    st.context = game.context(p)
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
                    _apply(game, p, st, idx, None)
                    continue
                by_model.setdefault(st.controller, []).append((g, p, enc, mask, phi, len(obs.board)))
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
            for unit in game.end_phase():
                place = game.placements[unit]
                assert place is not None
                for p in game.unit_players(unit):
                    traj = seats[g][p].traj
                    if traj is not None:
                        close(traj, placement_reward(place, config.units))

    results = []
    for g, (game, spec) in enumerate(zip(games, specs)):
        trajs = [s.traj.arrays() for s in seats[g] if s.traj is not None and s.traj.actions]
        results.append(
            GameResult(
                seed=spec.seed,
                controllers=list(spec.controllers),
                placements=[int(p) for p in game.placements],
                turns=game.turn,
                trajectories=trajs,
                replay=bytes(game.replay_bytes()) if opts.keep_replays else None,
            )
        )
    return results


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
    traj.slot_num_feats.append(enc[2])
    traj.action_mask.append(mask)
    traj.actions.append(action)
    traj.log_probs.append(log_prob)
    traj.values.append(value)
    traj.rewards.append(0.0)
    traj.turn_end.append(False)
    traj.potentials.append(phi)


def _apply(game: seaglass.LobbyGame, p: int, st: _Seat, idx: int, note: dict[str, Any] | None) -> None:
    """Take action `idx` for player `p`; mark the end of their turn on their trajectory."""
    done = game.step(p, action_from_index(idx), note)
    st.steps += 1
    if done and st.traj is not None and st.traj.actions:
        st.traj.turn_end[-1] = True


# -- Worker processes ---------------------------------------------------------------------------

_MODEL_CACHE: dict[str, BattlegroundsMLP] = {}
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
        if self.pool is None:
            return _play_chunk((config, specs, sources, options, record, seed))
        n = max(1, min(self.workers, len(specs)))
        chunks = [specs[i::n] for i in range(n)]
        jobs = [(config, c, sources, options, record, seed + 7919 * i) for i, c in enumerate(chunks)]
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
