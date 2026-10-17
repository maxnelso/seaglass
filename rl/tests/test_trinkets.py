"""Trinkets through the Rust<->Python seam (`engine/docs/trinkets.md` §8.2)."""

from __future__ import annotations

import numpy as np
import seaglass

from seaglass_rl import NUM_SLOTS, PLAYER_DIM, encode_observation
from seaglass_rl.actions import (
    ACTION_SPACE_SIZE,
    CHOOSE_TRINKET_INDICES,
    action_mask,
    index_from_action,
)


def test_trinket_actions_are_appended_last() -> None:
    n = len(CHOOSE_TRINKET_INDICES)
    assert CHOOSE_TRINKET_INDICES == list(range(ACTION_SPACE_SIZE - n, ACTION_SPACE_SIZE))
    act = seaglass.TavernAction.choose_trinket(2)
    assert act.kind == "choose_trinket" and act.args == (2, None)
    assert index_from_action(act) == CHOOSE_TRINKET_INDICES[2]


def test_the_trinket_table_has_the_pool() -> None:
    table = seaglass.trinkets_table()
    assert len(table) == 246
    assert sum(1 for (_, _, greater, _, _) in table if not greater) == 120
    ids = set(seaglass.all_card_ids())
    assert all(tid in ids for (tid, *_rest) in table)


def _play_to_turn(game: seaglass.TavernGame, turn: int) -> None:
    """End turns (without combat) until `turn` has started, taking any forced pick."""
    game.start_turn()
    while game.observe().turn < turn:
        while game.observe().trinket_options:
            game.step(seaglass.TavernAction.choose_trinket(0))
        game.end_turn()
        game.start_turn()


def test_a_game_without_trinkets_is_unchanged() -> None:
    game = seaglass.TavernGame(seed=7)
    _play_to_turn(game, 9)
    obs = game.observe()
    assert obs.trinkets == [] and obs.trinket_options == [] and obs.trinket_option_costs == []
    assert all(a.kind != "choose_trinket" for a in game.legal_actions())
    mask = action_mask(game, allow_reposition=True, allow_freeze=True)
    assert not np.any(mask[CHOOSE_TRINKET_INDICES])
    player_vec, slot_card_ids, _ = encode_observation(obs)
    assert player_vec.shape == (PLAYER_DIM,) and slot_card_ids.shape == (NUM_SLOTS,)
    assert not np.any(player_vec[-6:]) and not np.any(slot_card_ids[-6:])


def test_a_greater_offer_on_turn_9_blocks_everything_but_the_pick() -> None:
    game = seaglass.TavernGame(seed=5, trinkets=True)
    _play_to_turn(game, 9)
    obs = game.observe()
    assert obs.turn == 9 and 1 <= len(obs.trinket_options) <= 4
    assert len(obs.trinket_option_costs) == len(obs.trinket_options)
    assert min(obs.trinket_option_costs) <= 2 or len(obs.trinket_options) < 4
    legal = game.legal_actions()
    assert legal and all(a.kind == "choose_trinket" for a in legal)
    mask = action_mask(game, allow_reposition=True, allow_freeze=True)
    assert np.flatnonzero(mask).tolist() == [CHOOSE_TRINKET_INDICES[a.args[0]] for a in legal]
    player_vec, slot_card_ids, _ = encode_observation(obs)
    assert player_vec[-6] == 1.0 and slot_card_ids[-4] != 0

    pick = legal[0].args[0]
    chosen, cost, gold = obs.trinket_options[pick], obs.trinket_option_costs[pick], obs.gold
    owned = list(obs.trinkets)  # the Lesser one picked on turn 6, if there was an offer
    game.step(seaglass.TavernAction.choose_trinket(pick))
    obs = game.observe()
    assert obs.trinket_options == [] and obs.trinkets == owned + [chosen]
    names = {tid: name for (tid, name, *_rest) in seaglass.trinkets_table()}
    gained = 4 if names[chosen] == "Bob's Tip Jar" else 0  # "Gain 4 Gold"
    assert obs.gold == gold - cost + gained
    # A trinket that Discovers as it is picked (Book of Medivh, ...) leaves its prompt first.
    after = {a.kind for a in game.legal_actions()}
    assert "end_turn" in after or after == {"choose_discover"}
