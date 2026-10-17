"""Trinkets through the Rust<->Python seam (`engine/docs/trinkets.md` §8.2)."""

from __future__ import annotations

import numpy as np
import seaglass

from seaglass_rl import NUM_SLOTS, PLAYER_DIM, encode_observation

# The six trinket features of the player vector (`engine/docs/trinkets.md` §8.2), before the Dark
# Discovery button's two (`engine/docs/dark_gifts.md` §8.2).
TRINKET_PLAYER_FEATURES = slice(PLAYER_DIM - 8, PLAYER_DIM - 2)
from seaglass_rl.actions import (
    ACTION_SPACE_SIZE,
    CHOOSE_TRINKET_INDICES,
    DARK_DISCOVERY_INDEX,
    action_mask,
    index_from_action,
)


def test_trinket_actions_are_appended_last() -> None:
    n = len(CHOOSE_TRINKET_INDICES)
    # The trinket picks come last but for the Dark Discovery button (`engine/docs/dark_gifts.md`).
    end = DARK_DISCOVERY_INDEX
    assert CHOOSE_TRINKET_INDICES == list(range(end - n, end)) and end == ACTION_SPACE_SIZE - 1
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


def test_trinkets_are_on_by_default() -> None:
    # RL plays with trinkets (and Dark Gifts): the Python default (`engine/docs/trinkets.md` §8.2).
    game = seaglass.TavernGame(seed=7)
    _play_to_turn(game, 6)
    assert len(game.observe().trinket_options) == 4
    duos = seaglass.DuosTavernGame(seed=7)
    duos.start_turn()
    while duos.observe(0).turn < 6:
        for p in (0, 1):
            duos.end_turn(p)
        duos.start_turn()
    assert len(duos.observe(0).trinket_options) == 4


def test_a_game_without_trinkets_is_unchanged() -> None:
    game = seaglass.TavernGame(seed=7, trinkets=False)
    _play_to_turn(game, 9)
    obs = game.observe()
    assert obs.trinkets == [] and obs.trinket_options == [] and obs.trinket_option_costs == []
    assert all(a.kind != "choose_trinket" for a in game.legal_actions())
    mask = action_mask(game, allow_reposition=True, allow_freeze=True)
    assert not np.any(mask[CHOOSE_TRINKET_INDICES])
    player_vec, slot_card_ids, _ = encode_observation(obs)
    assert player_vec.shape == (PLAYER_DIM,) and slot_card_ids.shape == (NUM_SLOTS,)
    trinket_feats = player_vec[TRINKET_PLAYER_FEATURES]
    assert not np.any(trinket_feats) and not np.any(slot_card_ids[-6:])


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
    assert player_vec[TRINKET_PLAYER_FEATURES.start] == 1.0 and slot_card_ids[-4] != 0

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
