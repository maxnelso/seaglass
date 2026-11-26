"""Policy-guided search inside the turn (`search.py`)."""

from __future__ import annotations

import numpy as np
import pytest
import torch

from seaglass_rl import ladder, search
from seaglass_rl.actions import ACTION_SPACE_SIZE, END_TURN_INDEX, action_from_index, action_mask
from seaglass_rl.play import GameSpec, LobbyConfig, PlayOptions, Seat, play_games
from seaglass_rl.model import BattlegroundsMLP
from seaglass_rl.search import SearchSpec, line_budget, search_decision
from seaglass_rl.transformer import EntityTransformer

SMALL = dict(d_model=32, layers=2, heads=2, ff=64, head_dim=32, card_embed_dim=8)


def _model(seed: int = 0) -> EntityTransformer:
    torch.manual_seed(seed)
    m = EntityTransformer(seed=seed, kind_prior=True, **SMALL)
    m.eval()
    return m


def _shopping_game(seed: int = 3, turn: int = 3):
    """A lobby at the start of a turn-phase shop (some turns in, so there is a choice)."""
    from seaglass_rl.play import _bot

    game = LobbyConfig().make(seed)
    bots = [_bot("heuristic", p) for p in range(8)]
    while not game.done:
        game.begin_phase()
        if game.phase == "turn" and game.turn >= turn:
            return game
        while game.active_players():
            for p in game.active_players():
                seat = Seat(game, p)
                idx = bots[p].act(seat)
                seat.step(action_from_index(idx))
        game.end_phase()
    raise AssertionError("no such turn")


def _snapshot(game, p: int):
    obs = game.observe(p)
    legal = sorted((a.kind, *a.args) for a in game.legal_actions(p))
    return (
        obs.gold,
        obs.tavern_tier,
        obs.turn,
        [(u.card_id, u.attack, u.health) for u in obs.board],
        [(u.card_id, u.attack, u.health) for u in obs.hand],
        [(u.card_id, u.attack, u.health) for u in obs.shop],
        legal,
        len(game.replay_bytes()),
    )


def test_top_m_1_sims_1_is_the_policy_argmax() -> None:
    model, game = _model(), _shopping_game()
    p = game.active_players()[0]
    opts = PlayOptions()
    res = search_decision(model, game, p, opts, SearchSpec(top_m=1, sims=1), np.random.default_rng(0), 5)
    enc = search.features.encode_observation(game.observe(p), game.context(p), arranging=False)
    mask = action_mask(Seat(game, p), allow_reposition=False, allow_freeze=False, prune_dominated=False)
    logits, _ = search._forward(model, [enc], [mask])
    assert res.action == int(logits[0].argmax())
    assert res.lines == 0 and list(res.visits) == []


def test_result_is_legal_deterministic_and_bounded() -> None:
    model, game = _model(1), _shopping_game()
    opts = PlayOptions()
    for p in game.active_players()[:3]:
        legal = {int(i) for i in np.flatnonzero(action_mask(Seat(game, p)))}
        for spec in (SearchSpec(top_m=4, sims=8), SearchSpec(top_m=8, sims=16, gumbel_noise=True, max_line=6), SearchSpec(top_m=3, sims=1)):
            runs = [search_decision(model, game, p, opts, spec, np.random.default_rng(9), 17) for _ in range(2)]
            a, b = runs
            assert a.action == b.action and a.q == b.q and a.visits == b.visits and a.lines == b.lines
            assert np.array_equal(a.policy_target, b.policy_target)
            assert a.action in legal
            # The bound: `sims` lines, or the 1-per-candidate-per-phase minimum when sims is smaller.
            m = min(spec.top_m, len(legal))
            assert a.lines <= max(spec.sims, sum(left for left, _ in line_budget(spec, m)))
            assert a.lines == sum(left * per for left, per in line_budget(spec, m))
            t = a.policy_target
            assert t.dtype == np.float32 and t.shape == (ACTION_SPACE_SIZE,)
            assert abs(float(t.sum()) - 1.0) < 1e-5
            assert all(t[i] == 0 for i in range(ACTION_SPACE_SIZE) if i not in legal)
            assert -1.0 <= a.root_value <= 1.0 and a.forwards >= 2


def test_a_search_never_changes_the_game() -> None:
    model, game = _model(2), _shopping_game()
    p = game.active_players()[0]
    before = _snapshot(game, p)
    others = {q: _snapshot(game, q) for q in game.active_players()}
    search_decision(model, game, p, PlayOptions(), SearchSpec(top_m=4, sims=8), np.random.default_rng(0), 1)
    assert _snapshot(game, p) == before
    assert all(_snapshot(game, q) == s for q, s in others.items())


def test_mlp_models_search_too() -> None:
    game = _shopping_game()
    p = game.active_players()[0]
    res = search_decision(
        BattlegroundsMLP(seed=0, kind_prior=True), game, p, PlayOptions(), SearchSpec(top_m=2, sims=2), np.random.default_rng(0), 1
    )
    assert res.lines == 2


def _play(search_spec, record=("learner",), seed: int = 4, **opts):
    config = LobbyConfig(players=4, max_turns=5)
    specs = [GameSpec(seed=seed, controllers=["learner", "heuristic", "learner", "random"])]
    options = PlayOptions(search=search_spec, search_controllers=("learner",) if search_spec else (), **opts)
    return play_games(config, specs, {"learner": _model(5)}, options, record=record)


def test_play_games_with_search_records_the_policy_target() -> None:
    (res,) = _play(SearchSpec(top_m=2, sims=2, max_line=8))
    assert sorted(res.placements) == [1, 2, 3, 4]
    assert res.trajectories
    for tr in res.trajectories:
        n = len(tr["actions"])
        target = tr["search_policy"]
        assert target.shape == (n, ACTION_SPACE_SIZE) and target.dtype == np.float16
        searched = target.sum(axis=1) > 0.5
        assert searched.any() and not searched.all()  # the end-of-turn step is not searched
        mask = tr["action_mask"]
        assert (target[searched] > 0).sum() > 0
        assert not (target.astype(np.float32)[~mask] != 0).any()
        assert tr["log_probs"].max() <= 1e-6
    (packed,) = _play(SearchSpec(top_m=2, sims=2, max_line=8), pack=True)
    assert "search_policy" in packed.trajectories[0] and bool(packed.trajectories[0]["packed"])


def test_search_none_is_unchanged() -> None:
    (a,) = _play(None)
    (b,) = _play(None, record=("learner",))
    assert a.placements == b.placements and "search_policy" not in a.trajectories[0]
    # And a searching controller list without a spec changes nothing either.
    config = LobbyConfig(players=4, max_turns=5)
    specs = [GameSpec(seed=4, controllers=["learner", "heuristic", "learner", "random"])]
    (c,) = play_games(config, specs, {"learner": _model(5)}, PlayOptions(search_controllers=("learner",)))
    assert c.placements == a.placements
    for x, y in zip(a.trajectories, c.trajectories):
        assert np.array_equal(x["actions"], y["actions"])


def test_play_games_with_search_is_deterministic() -> None:
    (a,) = _play(SearchSpec(top_m=2, sims=2, max_line=8))
    (b,) = _play(SearchSpec(top_m=2, sims=2, max_line=8))
    assert a.placements == b.placements
    for x, y in zip(a.trajectories, b.trajectories):
        assert np.array_equal(x["actions"], y["actions"])


def test_search_is_solo_only() -> None:
    with pytest.raises(ValueError, match="Solo"):
        play_games(
            LobbyConfig(duos=True, players=4),
            [GameSpec(seed=1, controllers=["learner", "learner"])],
            {"learner": _model()},
            PlayOptions(search=SearchSpec(), search_controllers=("learner",)),
        )


TINY = dict(games=2, workers=0)


def _agent():
    m = _model(7)
    return (m.config, m.state_dict())


def test_ladder_rate_and_versus_with_search() -> None:
    cfg = LobbyConfig(max_turns=4)
    spec = SearchSpec(top_m=2, sims=2, max_line=6)
    res = ladder.rate(_agent(), games=2, workers=0, config=cfg, probe=None, search=spec)
    assert res["meta"]["search"]["top_m"] == 2
    assert "search" in ladder.format_report(res)
    with pytest.raises(ValueError):
        ladder.rate("heuristic", games=1, workers=0, config=cfg, probe=None, search=spec)
    out = ladder.versus(_agent(), _agent(), games=2, workers=0, config=cfg, search_a=spec)
    assert out["meta"]["games"] == 2 and len(out["lobbies"]) == 2
    assert 1 <= out["a"]["avg_place"] <= 8 and out["a"]["stderr"] >= 0
    assert "b" in out and "search" in ladder.format_versus(out)
    # Both sides at once also runs.
    out = ladder.versus(_agent(), _agent(), games=1, workers=0, config=cfg, search_a=spec, search_b=spec)
    assert len(out["lobbies"]) == 1


# ---- batching: the coroutine, the driver and `play_games` ----

import sys  # noqa: E402
from pathlib import Path  # noqa: E402

sys.path.insert(0, str(Path(__file__).parent))
import _frozen_search  # noqa: E402  (the pre-batching search_decision, kept as the reference)

from seaglass_rl.search import run_searches, search_coroutine  # noqa: E402

SPECS = (SearchSpec(top_m=4, sims=8), SearchSpec(top_m=8, sims=16, gumbel_noise=True, max_line=6))


def _same(a: search.SearchResult, b: search.SearchResult, exact: bool = True) -> None:
    assert a.action == b.action and a.visits == b.visits and a.lines == b.lines and a.forwards == b.forwards
    assert list(a.q) == list(b.q)
    for k in a.q:
        assert (a.q[k] == b.q[k]) if exact else abs(a.q[k] - b.q[k]) < 1e-5
    if exact:
        assert np.array_equal(a.policy_target, b.policy_target) and a.root_value == b.root_value
        assert a.log_prob == b.log_prob and a.v_mix == b.v_mix
    else:
        assert np.allclose(a.policy_target, b.policy_target, atol=1e-5)


def test_wrapper_matches_the_frozen_pre_batching_search() -> None:
    model = _model(1)
    for gseed, turn in ((3, 3), (5, 4)):
        game = _shopping_game(gseed, turn)
        for p in game.active_players()[:3]:
            for spec in SPECS:
                for seed in (17, 23):
                    new = search_decision(model, game, p, PlayOptions(), spec, np.random.default_rng(9), seed)
                    old = _frozen_search.search_decision(model, game, p, PlayOptions(), spec, np.random.default_rng(9), seed)
                    _same(new, old)


def test_run_searches_one_coroutine_is_search_decision() -> None:
    model, game = _model(2), _shopping_game()
    p = game.active_players()[1]
    spec = SPECS[1]
    (res,) = run_searches({"m": model}, [search_coroutine("m", game, p, PlayOptions(), spec, np.random.default_rng(4), 11)])
    _same(res, search_decision(model, game, p, PlayOptions(), spec, np.random.default_rng(4), 11))


def test_run_searches_many_coroutines_never_mix_rows() -> None:
    models = {"a": _model(1), "b": _model(2)}
    games = [_shopping_game(3, 3), _shopping_game(5, 4)]
    jobs = []
    for gi, game in enumerate(games):
        for p in game.active_players()[:3]:
            for ctrl in ("a", "b"):
                jobs.append((gi, p, ctrl, SPECS[(p + gi) % 2], 100 + p + 10 * gi))

    def coro(job):
        gi, p, ctrl, spec, seed = job
        return search_coroutine(ctrl, games[gi], p, PlayOptions(), spec, np.random.default_rng(seed), seed)

    together = run_searches(models, [coro(j) for j in jobs])
    assert len(together) == len(jobs)
    flips = 0
    for job, res in zip(jobs, together):
        gi, p, ctrl, spec, seed = job
        legal = {int(i) for i in np.flatnonzero(action_mask(Seat(games[gi], p)))}
        assert res.action in legal
        (alone,) = run_searches(models, [coro(job)])
        # Batch composition can change a model's float results in the last bits, which can (rarely)
        # flip a sampled line action or a halving tie; then lines diverge. Visits and lines come from
        # the budget alone, so they always agree; q agrees to 1e-5 unless a flip happened.
        assert res.visits == alone.visits and res.lines == alone.lines
        if res.action != alone.action:
            flips += 1
            continue
        _same(res, alone, exact=False)
    assert flips <= 1


def test_run_searches_batches_across_coroutines() -> None:
    model, game = _model(3), _shopping_game()
    calls = []
    real = model.forward
    model.forward = lambda *a, **k: (calls.append(len(a[0])), real(*a, **k))[1]
    spec = SearchSpec(top_m=4, sims=8, max_line=6)
    ps = game.active_players()[:4]

    def coros():
        return [search_coroutine("m", game, p, PlayOptions(), spec, np.random.default_rng(p), p) for p in ps]

    run_searches({"m": model}, coros())
    batched = len(calls)
    calls.clear()
    for c in coros():
        run_searches({"m": model}, [c])
    assert batched * 2 <= len(calls)


def test_play_games_batches_the_searching_seats() -> None:
    config = LobbyConfig(players=4, max_turns=4)
    specs = [GameSpec(seed=s, controllers=["learner", "heuristic", "learner", "random"]) for s in (1, 2, 3)]
    opts = PlayOptions(search=SearchSpec(top_m=3, sims=4, max_line=6), search_controllers=("learner",))
    counts = {}
    for name, size in (("all", 3), ("one at a time", 1)):
        model = _model(5)
        n = [0]
        real = model.forward
        model.forward = lambda *a, _real=real, _n=n, **k: (_n.__setitem__(0, _n[0] + 1), _real(*a, **k))[1]
        results = []
        for i in range(0, 3, size):
            results += play_games(config, specs[i : i + size], {"learner": model}, opts, record=("learner",))
        assert len(results) == 3
        for r in results:
            assert sorted(r.placements) == [1, 2, 3, 4] and r.trajectories
        counts[name] = n[0]
    assert counts["all"] * 1.3 <= counts["one at a time"]  # up to 3x: 3 lobbies share their batches


@pytest.mark.parametrize("horizon", ["next_start", "next_end"])
def test_search_past_the_turn_runs_and_is_deterministic(horizon: str) -> None:
    from seaglass_rl.probe import ProbeSpec

    spec = SearchSpec(top_m=3, sims=4, horizon=horizon, opponents=ProbeSpec(lobbies=4, per_turn=8, min_boards=4))
    config = LobbyConfig()
    specs = [GameSpec(seed=5, controllers=["a", "heuristic", "heuristic", "heuristic", "heuristic", "heuristic", "heuristic", "heuristic"])]
    opts = PlayOptions(search=spec, search_controllers=("a",), allow_reposition=False)

    def play():
        # A fresh model each time: a model's sampling rng is consumed as it plays.
        torch.manual_seed(0)
        model = EntityTransformer(seed=0, kind_prior=True, value_places=8, **SMALL).eval()
        return play_games(config, specs, {"a": model}, opts, record=())

    (r1,) = play()
    (r2,) = play()
    assert r1.placements == r2.placements and sorted(r1.placements) == list(range(1, 9))


def test_a_knocked_out_line_is_worth_the_last_place_left() -> None:
    from types import SimpleNamespace

    from seaglass_rl.search import _death_value

    assert _death_value(SimpleNamespace(units=8, alive_units=8)) == -1.0
    assert abs(_death_value(SimpleNamespace(units=8, alive_units=3)) - (1 - 2 * 2 / 7)) < 1e-12


def test_a_probe_library_from_a_network(tmp_path) -> None:
    from seaglass_rl import checkpoint
    from seaglass_rl.probe import ProbeSpec, build_library

    model = EntityTransformer(seed=0, kind_prior=True, value_places=8, **SMALL)
    path = tmp_path / "m.pt"
    checkpoint.save(path, model, iteration=0, args={"no_reposition": True})
    lib = build_library(ProbeSpec(lobbies=2, per_turn=4, min_boards=2, model=str(path)), LobbyConfig())
    assert lib and all(2 <= len(v) <= 4 for v in lib.values())
