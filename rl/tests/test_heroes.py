"""Heroes through the Rust<->Python seam (`engine/docs/heroes.md` §8)."""

from __future__ import annotations

import numpy as np
import seaglass

from seaglass_rl import PLAYER_DIM, encode_observation
from seaglass_rl.actions import (
    ACTION_SPACE_SIZE,
    CHOOSE_HERO_INDICES,
    HERO_POWER_INDICES,
    action_from_index,
    action_mask,
    index_from_action,
)


def test_hero_actions_are_appended_last() -> None:
    assert HERO_POWER_INDICES + CHOOSE_HERO_INDICES == list(
        range(ACTION_SPACE_SIZE - len(HERO_POWER_INDICES) - len(CHOOSE_HERO_INDICES), ACTION_SPACE_SIZE)
    )
    act = seaglass.TavernAction.hero_power(3)
    assert act.kind == "hero_power" and act.args == (3, None)
    assert index_from_action(seaglass.TavernAction.hero_power()) == HERO_POWER_INDICES[0]
    assert index_from_action(seaglass.TavernAction.choose_hero(1)) == CHOOSE_HERO_INDICES[1]


def test_a_game_without_a_hero_is_unchanged() -> None:
    game = seaglass.TavernGame(seed=7)
    game.start_turn()
    obs = game.observe()
    assert (obs.hero_id, obs.hero_power_id, obs.armor) == (0, 0, 5)
    assert not obs.hero_power_usable
    assert all(a.kind not in ("hero_power", "choose_hero") for a in game.legal_actions())
    mask = action_mask(game, allow_reposition=True, allow_freeze=True)
    assert not np.any(mask[HERO_POWER_INDICES + CHOOSE_HERO_INDICES])


def test_yshaarj_uses_its_hero_power_and_summons_in_combat() -> None:
    game = seaglass.TavernGame(seed=3, hero="Y'Shaarj")
    obs = game.observe()
    assert (obs.health, obs.armor, obs.hero_id, obs.hero_power_cost) == (30, 20, 66196, 2)
    game.start_turn()
    obs = game.observe()
    assert obs.hero_power_usable and obs.hero_power_uses_left == 1
    mask = action_mask(game)
    assert mask[HERO_POWER_INDICES[0]]
    for idx, is_valid in enumerate(mask):
        assert bool(is_valid) == game.is_legal(action_from_index(idx)) or idx < HERO_POWER_INDICES[0]

    game.step(action_from_index(HERO_POWER_INDICES[0]))
    obs = game.observe()
    assert (obs.gold, obs.hero_power_armed, obs.hero_power_uses_left) == (1, 1, 0)
    assert not game.is_legal(seaglass.TavernAction.hero_power())

    battle = game.resolve_combat_against([], seed=1)
    assert battle.outcome == "a_win"
    obs = game.observe()
    assert obs.hero_power_armed == 0
    assert len(obs.hand) == 1 and obs.hand[0].tavern_tier == 1


def test_a_targeted_hero_power_names_its_target() -> None:
    game = seaglass.TavernGame(seed=5, hero="Teron Gorefiend")
    game.start_turn()
    assert not game.is_legal(seaglass.TavernAction.hero_power(0))  # no minion yet
    while game.observe().gold >= 3 and not game.observe().board:
        game.step(seaglass.TavernAction.buy(0))
        game.step(seaglass.TavernAction.play(0, 0))
    assert game.is_legal(seaglass.TavernAction.hero_power(0))
    assert not game.is_legal(seaglass.TavernAction.hero_power())
    game.step(seaglass.TavernAction.hero_power(0))
    assert game.observe().hero_power_uses_left == 0


def test_hero_features_are_in_the_observation_encoding() -> None:
    game = seaglass.TavernGame(seed=3, hero=66196)
    game.start_turn()
    player_vec, _, _ = encode_observation(game.observe())
    assert player_vec.shape == (PLAYER_DIM,)
    assert player_vec[9] == 1.0 and player_vec[10] == 1.0  # a hero, its power usable


def test_a_duos_team_gets_both_heroes_duos_armor() -> None:
    game = seaglass.DuosTavernGame(seed=1, heroes=("Reno Jackson", "Y'Shaarj"))
    assert (game.health, game.armor) == (30, 28)
    obs = game.observe(1)
    assert (obs.hero_id, obs.teammate_hero_id) == (66196, 60212)
    patchwerk = seaglass.DuosTavernGame(seed=1, heroes=("Patchwerk", None))
    assert (patchwerk.health, patchwerk.armor) == (60, 0)


def test_the_hero_table() -> None:
    table = seaglass.heroes_table()
    assert len(table) == 123
    implemented = {name for _, name, done in table if done}
    assert {"Al'Akir", "Y'Shaarj", "Teron Gorefiend"} <= implemented
