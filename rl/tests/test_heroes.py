"""Heroes through the Rust<->Python seam (`engine/docs/heroes.md` §8)."""

from __future__ import annotations

import numpy as np
import seaglass

from seaglass_rl import PLAYER_DIM, encode_observation
from seaglass_rl.actions import (
    ACTION_SPACE_SIZE,
    CHOOSE_HERO_INDICES,
    HERO_POWER_2_INDICES,
    HERO_POWER_EXTRA_INDICES,
    HERO_POWER_INDICES,
    action_from_index,
    action_mask,
    index_from_action,
)


def test_hero_actions_are_appended_last() -> None:
    tail = HERO_POWER_INDICES + CHOOSE_HERO_INDICES + HERO_POWER_EXTRA_INDICES + HERO_POWER_2_INDICES
    assert tail == list(range(ACTION_SPACE_SIZE - len(tail), ACTION_SPACE_SIZE))
    act = seaglass.TavernAction.hero_power(3)
    assert act.kind == "hero_power" and act.args == (3, None)
    assert index_from_action(seaglass.TavernAction.hero_power()) == HERO_POWER_INDICES[0]
    assert index_from_action(seaglass.TavernAction.choose_hero(1)) == CHOOSE_HERO_INDICES[1]
    assert index_from_action(seaglass.TavernAction.hero_power(10)) == HERO_POWER_EXTRA_INDICES[0]
    assert index_from_action(seaglass.TavernAction.hero_power_2()) == HERO_POWER_2_INDICES[0]
    assert index_from_action(seaglass.TavernAction.hero_power_2(5)) == HERO_POWER_2_INDICES[6]


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


def test_cards_frozen_on_their_own_are_in_the_observation_encoding() -> None:
    from seaglass_rl.features import MAX_BOARD, MAX_HAND

    game = seaglass.TavernGame(seed=3, hero="Varden Dawngrasp")
    game.start_turn()
    obs = game.observe()
    frozen = [u.frozen for u in obs.shop]
    assert frozen.count(True) == 2  # the highest-Tier minion and its copy
    _, _, slot_num_feats = encode_observation(obs)
    shop = slot_num_feats[MAX_BOARD + MAX_HAND :][: len(frozen)]
    assert [bool(f[12]) for f in shop] == frozen
    game.step(seaglass.TavernAction.toggle_freeze())
    assert not any(u.frozen for u in game.observe().shop)
    assert not game.observe().is_frozen


def test_the_next_minion_discount_is_in_the_observation() -> None:
    game = seaglass.TavernGame(seed=1, hero="Aranna Starseeker")
    game.start_turn()
    obs = game.observe()
    assert obs.next_minion_discount == 0
    player_vec, _, _ = encode_observation(obs)
    assert player_vec.shape == (PLAYER_DIM,) and player_vec[16] == 0.0


def test_hero_prices_and_marks_are_in_the_observation() -> None:
    # `engine/docs/tavern.md` §3.3: Millhouse's minions and Refreshes cost 2, his Upgrade 1 more.
    game = seaglass.TavernGame(seed=1, hero="Millhouse Manastorm")
    game.start_turn()
    obs = game.observe()
    assert (obs.minion_cost, obs.refresh_cost, obs.upgrade_cost) == (2, 2, 6)
    player_vec, _, slot_num_feats = encode_observation(obs)
    assert player_vec[18] == np.float32(2 / 3) and player_vec[19] == 1.0
    plain = seaglass.TavernGame(seed=1)
    plain.start_turn()
    obs = plain.observe()
    assert (obs.minion_cost, obs.refresh_cost, obs.upgrade_cost) == (3, 1, 5)
    # A Silas Darkmoon ticket marks a minion in the Tavern.
    silas = seaglass.TavernGame(seed=3, hero="Silas Darkmoon")
    silas.start_turn()
    shop = silas.observe().shop
    marked = sum(u.marked for u in shop)
    assert marked >= 1
    _, _, slot_num_feats = encode_observation(silas.observe())
    assert slot_num_feats[:, 13].sum() == float(marked)


def test_a_skipped_turn_only_allows_ending_it() -> None:
    game = seaglass.TavernGame(seed=1, hero="A. F. Kay")
    game.start_turn()
    assert [a.kind for a in game.legal_actions()] == ["end_turn"]
    mask = action_mask(game, allow_reposition=True, allow_freeze=True)
    assert mask.sum() == 1


def test_second_hero_power_and_delegation_heroes_in_the_observation() -> None:
    # Master Nguyen offers 2 Hero Power options at Turn 1 start, both in the vocabulary.
    nguyen = seaglass.TavernGame(seed=7, hero="Master Nguyen")
    nguyen.start_turn()
    obs = nguyen.observe()
    assert obs.discover_pending is not None and len(obs.discover_pending) == 2
    _, slot_card_ids, _ = encode_observation(obs)
    assert slot_card_ids[24] > 0 and slot_card_ids[25] > 0

    # Genn, Worgen King triggers King of Duality on Turn 4 and gains a second Hero Power.
    genn = seaglass.TavernGame(seed=5, hero="Genn, Worgen King")
    for _ in range(3):
        genn.start_turn()
        genn.step(seaglass.TavernAction.end_turn())
    genn.start_turn()
    assert genn.observe().second_hero_power_id == 0
    genn.step(seaglass.TavernAction.choose_discover(0))
    genn.step(seaglass.TavernAction.choose_discover(0))
    obs = genn.observe()
    assert obs.second_hero_power_id != 0
    player_vec, _, _ = encode_observation(obs)
    assert player_vec[20] == 1.0

    # The Nameless One copies its teammate's Hero Power on Turn 1 start in Duos.
    duos = seaglass.DuosTavernGame(seed=3, heroes=("The Nameless One", "Y'Shaarj"))
    duos.start_turn()
    assert duos.observe(0).hero_power_id == duos.observe(1).hero_power_id



def test_voljins_first_pick_is_observed_where_it_is_now() -> None:
    # `engine/docs/heroes.md` §8: `hero_power_armed` is the picked minion's current target + 1.
    game = seaglass.TavernGame(seed=3, hero="Vol'jin")
    game.start_turn()
    game.step(seaglass.TavernAction.end_turn())
    game.start_turn()
    assert game.observe().gold == 4
    shop = game.observe().shop
    minions = [i for i, u in enumerate(shop) if not u.is_spell]
    assert len(minions) >= 2
    picked = minions[1]
    game.step(seaglass.TavernAction.hero_power(7 + picked))
    obs = game.observe()
    assert (obs.hero_power_armed, obs.hero_power_uses_left) == (8 + picked, 1)
    game.step(seaglass.TavernAction.buy(minions[0]))
    assert game.observe().hero_power_armed == 7 + picked  # one slot to the left
    game.step(seaglass.TavernAction.refresh())
    assert game.observe().hero_power_armed == 0  # the pick dropped


def _units(*names: str) -> list:
    templates = {t.name: t for t in seaglass.catalog("full")}
    return [templates[name].instantiate() for name in names]


def _head_to_head(hero: str, opponent_board: list) -> tuple:
    """A game with `hero` against a plain one whose last warband was `opponent_board`, each the
    other's last and next opponent, at the start of turn 1."""
    game = seaglass.TavernGame(seed=4, hero=hero)
    opp = seaglass.TavernGame(seed=5)
    lobby = seaglass.Lobby(2)
    lobby.record_combat(0, 1, [])
    lobby.record_combat(1, 0, opponent_board)
    lobby.set_next_pairs([(0, 1)])
    lobby.apply([game, opp])
    game.start_turn()
    opp.start_turn()
    lobby.deliver_gifts([game, opp])
    return game, opp, lobby


def test_scabbs_discovers_from_the_opponents_last_warband() -> None:
    game, _, _ = _head_to_head("Scabbs Cutterbutter", _units("Joyous", "Zoatroid"))
    game.step(seaglass.TavernAction.hero_power())
    options = game.observe().discover_pending
    assert options is not None and {u.name for u in options} == {"Joyous", "Zoatroid"}
    game.step(seaglass.TavernAction.choose_discover(0))
    assert game.observe().hand[0].name in {"Joyous", "Zoatroid"}


def test_tess_refreshes_with_the_opponents_last_warband() -> None:
    game, _, _ = _head_to_head("Tess Greymane", _units("Joyous", "Zoatroid"))
    game.step(seaglass.TavernAction.hero_power())
    minions = [u.name for u in game.observe().shop if not u.is_spell]
    assert sorted(minions) == ["Joyous", "Zoatroid"]


def test_mukla_gives_the_opponent_a_banana() -> None:
    _, opp, _ = _head_to_head("King Mukla", [])
    assert [u.name for u in opp.observe().hand] in (["Bananas"], ["Big Banana"])


def test_holmes_guess_gives_a_coin() -> None:
    game, _, _ = _head_to_head("Murloc Holmes", _units("Joyous"))
    game.step(seaglass.TavernAction.hero_power())
    assert [u.name for u in game.observe().discover_pending] == ["Joyous"]
    game.step(seaglass.TavernAction.choose_discover(0))
    assert [u.name for u in game.observe().hand] == ["Tavern Coin"]


def test_the_lobby_is_not_in_the_observation() -> None:
    # The player doesn't see the lobby: the same Tavern with and without one encodes the same.
    plain = seaglass.TavernGame(seed=4)
    plain.start_turn()
    game = seaglass.TavernGame(seed=4)
    others = [seaglass.TavernGame(seed=6), seaglass.TavernGame(seed=7)]
    lobby = seaglass.Lobby(3)
    lobby.record_combat(0, 1, _units("Joyous"))
    lobby.record_combat(1, 0, _units("Joyous", "Zoatroid"))
    lobby.record_combat(2, None, _units("Wrath Weaver"))
    lobby.record_outcome(0, 1, "a_win")
    lobby.record_deaths([True, True, True])
    lobby.set_next_pairs([(1, 2)])
    lobby.apply([game, *others])
    game.start_turn()
    assert not hasattr(game.observe(), "lobby")
    for a, b in zip(encode_observation(plain.observe()), encode_observation(game.observe())):
        assert np.array_equal(a, b)
