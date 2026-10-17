"""Dark Gifts through the Rust<->Python seam (`engine/docs/dark_gifts.md` §8.2)."""

from __future__ import annotations

import numpy as np
import seaglass

from seaglass_rl import PLAYER_DIM, SLOT_NUM_DIM, encode_observation
from seaglass_rl.actions import ACTION_SPACE_SIZE, DARK_DISCOVERY_INDEX, action_mask, index_from_action
from seaglass_rl.features import BASE_SLOT_NUM_DIM, DARK_GIFT_IDS


def test_the_dark_discovery_action_is_appended_last() -> None:
    assert DARK_DISCOVERY_INDEX == ACTION_SPACE_SIZE - 1
    act = seaglass.TavernAction.dark_discovery()
    assert act.kind == "dark_discovery" and act.args == (None, None)
    assert index_from_action(act) == DARK_DISCOVERY_INDEX


def test_the_gift_table_and_vocabulary() -> None:
    table = seaglass.dark_gifts_table()
    assert len(table) == 47
    assert sorted(DARK_GIFT_IDS) == sorted(gid for (gid, *_rest) in table)
    ids = seaglass.all_card_ids()
    assert ids[-len(DARK_GIFT_IDS):] == DARK_GIFT_IDS
    assert SLOT_NUM_DIM == BASE_SLOT_NUM_DIM + 47


def _play_to_turn(game: seaglass.TavernGame, turn: int) -> None:
    game.start_turn()
    while game.observe().turn < turn:
        game.end_turn()
        game.start_turn()


def test_dark_gifts_are_on_by_default() -> None:
    # RL plays with Dark Gifts (and trinkets): the Python default (`engine/docs/dark_gifts.md` §8.2).
    for game in (seaglass.TavernGame(seed=7), seaglass.DuosTavernGame(seed=7)):
        obs = game.observe() if isinstance(game, seaglass.TavernGame) else game.observe(0)
        assert obs.dark_gifts_enabled and obs.dark_discoveries_left == 3


def test_a_game_without_dark_gifts_is_unchanged() -> None:
    game = seaglass.TavernGame(seed=7, dark_gifts=False)
    _play_to_turn(game, 4)
    obs = game.observe()
    assert not obs.dark_gifts_enabled and obs.dark_discoveries_left == 0
    assert not obs.dark_discovery_usable
    assert all(a.kind != "dark_discovery" for a in game.legal_actions())
    assert not action_mask(game)[DARK_DISCOVERY_INDEX]
    player_vec, _, slot_num_feats = encode_observation(obs)
    assert player_vec.shape == (PLAYER_DIM,) and not np.any(player_vec[-2:])
    assert not np.any(slot_num_feats[:, BASE_SLOT_NUM_DIM:])


def test_the_button_discovers_a_minion_with_a_gift() -> None:
    game = seaglass.TavernGame(seed=3, dark_gifts=True)
    _play_to_turn(game, 3)
    obs = game.observe()
    assert obs.dark_gifts_enabled and obs.dark_discoveries_left == 3 and obs.dark_discovery_usable
    assert action_mask(game)[DARK_DISCOVERY_INDEX]
    player_vec, _, _ = encode_observation(obs)
    assert player_vec[-2] == 1.0 and player_vec[-1] == 1.0
    gold = obs.gold
    game.step(seaglass.TavernAction.dark_discovery())
    obs = game.observe()
    assert obs.gold == gold - 3 and obs.dark_discoveries_left == 2 and not obs.dark_discovery_usable
    options = obs.discover_pending
    assert options and 1 <= len(options) <= 3
    gifts = [u.dark_gifts for u in options]
    assert all(len(g) == 1 for g in gifts) and len({g[0] for g in gifts}) == len(gifts)
    _, _, slot_num_feats = encode_observation(obs)
    discover = slice(7 + 10 + 7, 7 + 10 + 7 + len(options))
    assert np.all(slot_num_feats[discover, BASE_SLOT_NUM_DIM:].sum(axis=1) == 1.0)
    game.step(seaglass.TavernAction.choose_discover(0))
    assert game.observe().hand[-1].dark_gifts == gifts[0]
