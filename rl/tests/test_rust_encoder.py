"""The Rust encoder (`seaglass.Encoder`) against the Python reference (`features.encode_observation`,
`actions.action_mask`): bit-identical arrays at every decision of whole lobbies, and the same games
through `play_games` with `PlayOptions(rust_encoder=True)` and `False`."""

from __future__ import annotations

import itertools

import numpy as np
import pytest
import seaglass
import torch

from seaglass_rl import features
from seaglass_rl.actions import action_from_index, action_mask
from seaglass_rl.features import encode_observation, fast_encode, fast_mask
from seaglass_rl.model import BattlegroundsMLP
from seaglass_rl.play import BOTS, GameSpec, LobbyConfig, PlayOptions, Seat, _bot, play_games

pytestmark = pytest.mark.skipif(not features.HAS_RUST_ENCODER, reason="the extension has no seaglass.Encoder")

FLAGS = list(itertools.product([False, True], repeat=3))

# Decisions compared, by kind of lobby (read with `pytest -s`).
CHECKED = {"solo": 0, "duos": 0}


def _same(label: str, a: np.ndarray, b: np.ndarray) -> None:
    assert a.dtype == b.dtype and a.shape == b.shape, f"{label}: {a.dtype}{a.shape} vs {b.dtype}{b.shape}"
    if np.array_equal(a, b):
        return
    bad = np.argwhere(a != b)
    lines = [f"{label}: {len(bad)} entries differ (rust first, python second)"]
    for idx in bad[:8]:
        idx = tuple(int(i) for i in idx)
        lines.append(f"  at {idx}: {a[idx]!r} vs {b[idx]!r}")
    raise AssertionError("\n".join(lines))


def _check(enc, game: seaglass.LobbyGame, p: int, kind: str) -> None:
    seat = Seat(game, p)
    obs = seat.observe()
    in_turn = game.phase == "turn"
    ctx = game.context(p) if in_turn else None
    for arranging in (False, True):
        want = encode_observation(obs, ctx, arranging=arranging)
        got = fast_encode(enc, game, p, arranging, in_turn)
        for name, a, b in zip(("player_vec", "slot_card_ids", "slot_num_feats"), got, want):
            _same(f"{kind} p{p} turn {game.turn} {name} arranging={arranging}", a, b)
        # The sparse form: exactly the nonzeros of the dense buffer (-0.0 is a zero).
        sp = fast_encode(enc, game, p, arranging, in_turn, sparse=True)
        assert len(sp) == 5
        for name, a, b in zip(("player_vec", "slot_card_ids", "slot_num_feats"), sp, got):
            _same(f"{kind} p{p} turn {game.turn} sparse {name}", a, b)
        flat = got[2].reshape(-1)
        cols = np.nonzero(flat)[0]
        _same("sparse idx", sp[3], cols.astype(np.uint16))
        _same("sparse val", sp[4], flat[cols])
    for rep, frz, prune in FLAGS:
        want = action_mask(seat, rep, frz, prune, obs=obs)
        got = fast_mask(enc, game, p, rep, frz, prune)
        _same(f"{kind} p{p} turn {game.turn} mask {(rep, frz, prune)}", got, want)
    assert enc.board_len(game, p) == len(obs.board)
    CHECKED[kind] += 1


def _drive(config: LobbyConfig, seed: int, controllers: list[str]) -> int:
    """Play a whole lobby by hand, comparing the encoders at every decision of every player."""
    enc = features.rust_encoder()
    game = config.make(seed)
    kind = "duos" if config.duos else "solo"
    bots = [_bot(c, seed * 16 + p) for p, c in enumerate(controllers)]
    seats = [Seat(game, p) for p in range(config.players)]
    n = 0
    while not game.done:
        game.begin_phase()
        steps = {p: 0 for p in range(config.players)}
        while True:
            active = game.active_players()
            if not active:
                break
            for p in active:
                bot = bots[game.unit_of(p)]
                if steps[p] >= 60:
                    game.force_end(p)
                    continue
                _check(enc, game, p, kind)
                n += 1
                steps[p] += 1
                idx = bot.act(seats[p])
                if not seats[p].is_legal(action_from_index(idx)):
                    game.force_end(p)
                    continue
                seats[p].step(action_from_index(idx))
        game.end_phase()
    return n


SOLO_MIXES = [
    ["heuristic"] * 8,
    ["random"] * 8,
    ["9kgamerbot", "9kgamerbot-tempo", "heuristic", "random"] * 2,
    ["9kgamerbot-tempo"] * 4 + ["heuristic"] * 4,
]


@pytest.mark.parametrize("seed", range(20))
def test_solo_parity_at_every_decision(seed: int) -> None:
    mix = SOLO_MIXES[seed % len(SOLO_MIXES)]
    n = _drive(LobbyConfig(), seed, mix[seed % 3 :] + mix[: seed % 3])
    assert n > 100


@pytest.mark.parametrize("seed", range(6))
def test_duos_parity_at_every_decision(seed: int) -> None:
    mix = ["heuristic", "random"] * 2 if seed % 2 else ["heuristic"] * 4
    n = _drive(LobbyConfig(duos=True), 100 + seed, mix)
    assert n > 100


def test_a_lobby_with_every_tribe_and_one_without_heroes() -> None:
    assert _drive(LobbyConfig(tribes=0), 31, ["heuristic", "random"] * 4) > 100
    assert _drive(LobbyConfig(heroes=False), 32, ["9kgamerbot", "random"] * 4) > 100


def test_the_parity_test_covers_the_rarer_features() -> None:
    """The lobbies above should have met trinkets, Dark Gifts, goldens and spells (a guard against
    the parity test going vacuous)."""
    seen = set()
    for seed in range(4):
        game = LobbyConfig().make(500 + seed)
        bot = _bot("9kgamerbot", seed)
        while not game.done:
            game.begin_phase()
            if game.phase == "turn":
                for p in game.active_players():
                    seat = Seat(game, p)
                    for _ in range(40):
                        if game.is_done(p):
                            break
                        obs = seat.observe()
                        units = [*obs.board, *obs.hand, *obs.shop]
                        seen |= {"trinket"} if obs.trinkets else set()
                        seen |= {"gift"} if any(u.dark_gifts for u in units) else set()
                        seen |= {"golden"} if any(u.is_golden for u in units) else set()
                        seen |= {"spell"} if any(u.is_spell for u in units) else set()
                        seen |= {"discover"} if obs.discover_pending else set()
                        idx = bot.act(seat)
                        if not seat.is_legal(action_from_index(idx)):
                            break
                        seat.step(action_from_index(idx))
            game.end_phase()
    assert {"trinket", "gift", "golden", "spell", "discover"} <= seen, seen


def _sig(results) -> list:
    out = []
    for r in results:
        out.append((r.placements, r.turns))
        for tr in r.trajectories:
            out.append({k: np.array(v, copy=True) for k, v in tr.items()})
    return out


def _same_results(a, b) -> None:
    assert len(a) == len(b)
    for x, y in zip(a, b):
        if isinstance(x, tuple):
            assert x == y
        else:
            assert x.keys() == y.keys()
            for k in x:
                assert np.array_equal(x[k], y[k]), k


@pytest.mark.parametrize("duos", [False, True])
@pytest.mark.parametrize("opts", [{}, {"shaping": 0.3, "health_shaping": 0.2}, {"positioning_phase": False}])
def test_play_games_is_identical_with_and_without_the_rust_encoder(duos: bool, opts: dict) -> None:
    config = LobbyConfig(duos=duos)
    controllers = ["learner", "9kgamerbot", "learner", "heuristic", "random", "learner", "heuristic", "random"][
        : config.units
    ]
    specs = [GameSpec(seed=s, controllers=controllers) for s in (4, 5)]
    out = []
    for rust in (False, True):
        torch.manual_seed(7)  # the same initial weights; the model's sampler has its own seed
        out.append(
            _sig(
                play_games(
                    config,
                    specs,
                    {"learner": BattlegroundsMLP(seed=3, kind_prior=True)},
                    PlayOptions(rust_encoder=rust, **opts),
                    record=("learner", "9kgamerbot", "heuristic"),
                )
            )
        )
    _same_results(*out)
    assert any(isinstance(x, dict) and len(x["actions"]) > 20 for x in out[1])


def test_the_encoder_rejects_a_bad_layout() -> None:
    spec = features.encoder_spec()
    seaglass.Encoder(spec)  # the good one builds
    for name, delta in [("NUM_SLOTS", 1), ("HERO_OFFSET", -1), ("PLAYER_DIM", 3), ("COUNTERS_DIM", 1)]:
        bad = features.encoder_spec()
        bad["layout"][name] += delta
        with pytest.raises(ValueError):
            seaglass.Encoder(bad)
    bad = features.encoder_spec()
    del bad["layout"]["MAX_SHOP"]
    with pytest.raises(ValueError, match="MAX_SHOP"):
        seaglass.Encoder(bad)
    bad = features.encoder_spec()
    bad["actions"] = bad["actions"][:-1]
    with pytest.raises(ValueError):
        seaglass.Encoder(bad)
    bad = features.encoder_spec()
    bad["actions"][1] = bad["actions"][0]
    with pytest.raises(ValueError, match="duplicate"):
        seaglass.Encoder(bad)
    with pytest.raises(ValueError, match="missing"):
        seaglass.Encoder({})


def test_sparse_encode_defaults_and_zero_handling() -> None:
    enc = features.rust_encoder()
    game = LobbyConfig().make(7)
    game.begin_phase()
    p = game.active_players()[0]
    assert len(enc.encode(game, p, False, True)) == 3
    assert len(enc.encode(game, p, False, True, False)) == 3
    out = enc.encode(game, p, False, True, True)
    assert len(out) == 5 and len(out[3]) % 2 == 0 and len(out[4]) == 2 * len(out[3])
    idx = np.frombuffer(out[3], dtype=np.uint16)
    assert (np.diff(idx.astype(np.int64)) > 0).all() and len(idx) > 0


def _check_tavern(enc, tavern, ctx, rng: np.random.Generator) -> None:
    obs = tavern.observe()
    for arranging in (False, True):
        want = encode_observation(obs, ctx, arranging=arranging)
        got = features.fast_encode_tavern(enc, tavern, ctx, arranging)
        for name, a, b in zip(("player_vec", "slot_card_ids", "slot_num_feats"), got, want):
            _same(f"tavern {name} arranging={arranging}", a, b)
        # Without a context (the lobby features are zeros).
        want = encode_observation(obs, None, arranging=arranging)
        got = features.fast_encode_tavern(enc, tavern, None, arranging)
        for name, a, b in zip(("player_vec", "slot_card_ids", "slot_num_feats"), got, want):
            _same(f"tavern (no context) {name}", a, b)
    for rep, frz, prune in FLAGS:
        _same(
            f"tavern mask {(rep, frz, prune)}",
            features.fast_mask_tavern(enc, tavern, rep, frz, prune),
            action_mask(tavern, rep, frz, prune),
        )


def test_tavern_parity_over_belief_taverns() -> None:
    """`encode_tavern` and `mask_tavern` of a belief `TavernGame`, stepped through random legal actions
    (refreshes, Discovers, plays, spells, trinkets...), equal the Python reference."""
    assert hasattr(features.rust_encoder(), "encode_tavern")
    enc = features.rust_encoder()
    rng = np.random.default_rng(11)
    checked = 0
    for seed in range(6):
        game = LobbyConfig().make(900 + seed)
        bots = [_bot("9kgamerbot" if p % 2 else "heuristic", seed * 16 + p) for p in range(8)]
        while not game.done and checked < 4000 * (seed + 1) // 6:
            game.begin_phase()
            if game.phase == "turn" and game.turn >= 2:
                for p in game.active_players()[:2]:
                    ctx = game.context(p)
                    for line in range(2):
                        tav = game.belief_tavern(p, seed=seed * 1000 + p * 10 + line)
                        for _ in range(25):
                            _check_tavern(enc, tav, ctx, rng)
                            checked += 1
                            legal = tav.legal_actions()
                            if not legal:
                                break
                            act = legal[int(rng.integers(len(legal)))]
                            if act.kind == "end_turn":
                                continue
                            tav.step(act)
            # Play the live turn with the bots.
            steps = {p: 0 for p in range(8)}
            while True:
                active = game.active_players()
                if not active:
                    break
                for p in active:
                    seat = Seat(game, p)
                    idx = bots[p].act(seat) if steps[p] < 40 else None
                    steps[p] += 1
                    if idx is None or not seat.is_legal(action_from_index(idx)):
                        game.force_end(p)
                    else:
                        seat.step(action_from_index(idx))
            game.end_phase()
    assert checked >= 3000, checked
