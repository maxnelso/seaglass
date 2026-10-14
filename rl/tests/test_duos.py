"""Duos through the Rust<->Python seam: a seeded team game driven from Python."""

from __future__ import annotations

import random

import numpy as np
import seaglass

from seaglass_rl.actions import (
    ACTION_SPACE_SIZE,
    MAX_HAND,
    PASS_INDICES,
    action_from_index,
    action_mask,
    index_from_action,
)


def _finish_turn(game: seaglass.DuosTavernGame, player: int) -> None:
    """End `player`'s turn, choosing the first option of any pending Discover first."""
    while not game.observe(player).ended:
        if game.observe(player).discover_pending is not None:
            game.step(player, seaglass.TavernAction.choose_discover(0))
        else:
            game.end_turn(player)


def _play_turn(game: seaglass.DuosTavernGame, rng: random.Random, trace: list, steps: int = 12) -> None:
    """Both players take random legal actions, interleaved, with a Pass now and then."""
    for _ in range(steps):
        for player in (0, 1):
            legal = [a for a in game.legal_actions(player) if a.kind != "end_turn"]
            if not legal:
                continue
            passes = [a for a in legal if a.kind == "pass"]
            act = rng.choice(passes) if passes and rng.random() < 0.35 else rng.choice(legal)
            game.step(player, act)
            obs = game.observe(player)
            trace.append(
                (player, act.kind, act.args, obs.gold, obs.team_health, len(obs.hand), len(obs.incoming))
            )
    for player in (0, 1):
        _finish_turn(game, player)
    assert game.turn_over()


def _run(seed: int, turns: int = 4) -> tuple[list, seaglass.DuosTavernGame, seaglass.DuosTavernGame]:
    """Two seeded teams play `turns` turns of random legal actions, fighting after each."""
    rng = random.Random(seed)
    team_a = seaglass.DuosTavernGame(seed=seed)
    team_b = seaglass.DuosTavernGame(seed=seed + 1)
    trace: list = []
    for turn in range(turns):
        for game in (team_a, team_b):
            game.start_turn()
            _play_turn(game, rng, trace)
        result = team_a.resolve_combat_with(team_b, seed=seed * 100 + turn)
        trace.append(
            (
                result.outcome,
                result.hero_damage,
                tuple(result.fought_a),
                tuple(result.fought_b),
                team_a.health,
                team_b.health,
            )
        )
    return trace, team_a, team_b


def test_a_duos_game_is_reproducible_from_its_seed() -> None:
    first, _, _ = _run(seed=5)
    second, _, _ = _run(seed=5)
    assert first == second
    other, _, _ = _run(seed=6)
    assert other != first


def test_a_duos_game_passes_cards_and_fights_as_a_team() -> None:
    trace, team_a, team_b = _run(seed=11)
    assert any(step[1] == "pass" for step in trace if len(step) == 7)
    fights = [step for step in trace if len(step) == 6]
    assert len(fights) == 4
    assert all(outcome in ("a_win", "b_win", "draw") for outcome, *_ in fights)
    for team in (team_a, team_b):
        obs = [team.observe(0), team.observe(1)]
        # One Health pool: both players see the team's.
        assert obs[0].health == obs[1].health == obs[0].team_health == team.health
        assert obs[0].armor == obs[1].armor == team.armor
        assert obs[0].passes_this_game == obs[1].passes_this_game
    assert team_a.health <= 30 and team_b.health <= 30


def _second_turn(seed: int) -> seaglass.DuosTavernGame:
    """A team on its second turn (4 Gold each), where player 0 bought a minion (1 Gold left)."""
    game = seaglass.DuosTavernGame(seed=seed, shop_spells=False)
    game.start_turn()
    for player in (0, 1):
        game.end_turn(player)
    game.start_turn()
    game.step(0, seaglass.TavernAction.buy(0))
    return game


def test_the_duos_observation_shows_the_teammate_and_the_team() -> None:
    game = seaglass.DuosTavernGame(seed=3, armor=(4, 2))
    game.start_turn()
    obs = [game.observe(0), game.observe(1)]
    assert obs[0].team_health == 30 and obs[0].team_armor == 6
    assert obs[0].fights_first != obs[1].fights_first
    assert obs[0].first_fighter == obs[1].first_fighter
    assert [u.card_id for u in obs[0].teammate_shop] == [u.card_id for u in obs[1].shop]
    assert obs[0].teammate_gold == obs[1].gold == 3
    assert obs[0].free_passes == 0 and obs[0].incoming == []


def test_a_pass_moves_a_card_and_counts_for_the_team() -> None:
    game = _second_turn(seed=1)
    bought = game.observe(0).hand[0].card_id
    assert game.is_legal(0, seaglass.TavernAction.pass_card(0))
    game.step(0, seaglass.TavernAction.pass_card(0))
    obs = [game.observe(0), game.observe(1)]
    assert obs[0].hand == [] and obs[0].gold == 0
    assert [u.card_id for u in obs[1].hand] == [bought]
    assert obs[0].passes_this_turn == obs[1].passes_this_turn == 1
    assert not game.is_legal(0, seaglass.TavernAction.pass_card(0))


def test_the_duos_action_mask_matches_engine_legality() -> None:
    for seed in range(5):
        game = _second_turn(seed)
        for player in (0, 1):
            mask = action_mask(game, allow_reposition=True, allow_freeze=True, player=player)
            assert mask.shape == (ACTION_SPACE_SIZE,)
            for idx, is_valid in enumerate(mask):
                assert bool(is_valid) == game.is_legal(player, action_from_index(idx))
        assert action_mask(game, player=0)[PASS_INDICES[0]]
        assert not action_mask(game, player=1)[PASS_INDICES[0]]  # an empty hand


def test_passes_are_appended_last_and_never_legal_in_solo() -> None:
    assert PASS_INDICES == list(range(ACTION_SPACE_SIZE - MAX_HAND, ACTION_SPACE_SIZE))
    assert index_from_action(seaglass.TavernAction.pass_card(3)) == PASS_INDICES[3]
    act = seaglass.TavernAction.pass_card(2)
    assert act.kind == "pass" and act.args == (2, None)

    game = seaglass.TavernGame(seed=7, catalog="full")
    game.start_turn()
    game.step(seaglass.TavernAction.buy(0))
    assert not game.is_legal(seaglass.TavernAction.pass_card(0))
    assert all(a.kind != "pass" for a in game.legal_actions())
    mask = action_mask(game, allow_reposition=True, allow_freeze=True)
    assert not np.any(mask[PASS_INDICES])


def test_simulate_duos_and_its_batch() -> None:
    team_a = [[seaglass.parse_unit("3/3"), seaglass.parse_unit("1/1")], [seaglass.parse_unit("5/5")]]
    team_b = [[seaglass.parse_unit("2/2")], [seaglass.parse_unit("4/6")]]
    result = seaglass.simulate_duos(team_a, team_b, seed=0, hero_tiers_a=(2, 4), hero_tiers_b=(3, 5))
    assert result.outcome in ("a_win", "b_win", "draw")
    assert len(result.fought_a) == 2 and len(result.survivors_b) == 2
    again = seaglass.simulate_duos(team_a, team_b, seed=0, hero_tiers_a=(2, 4), hero_tiers_b=(3, 5))
    assert (again.outcome, again.hero_damage) == (result.outcome, result.hero_damage)

    dist = seaglass.simulate_duos_batch(team_a, team_b, base_seed=1, n=16)
    assert dist.battles == 16 == dist.a_wins + dist.b_wins + dist.draws
