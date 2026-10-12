"""Discrete action space mapping and legality masking over `seaglass.TavernAction`."""

from __future__ import annotations

import numpy as np
import seaglass

MAX_BOARD = 7
MAX_HAND = 10
MAX_SHOP = 7
MAX_DISCOVER = 4


def _build_action_table() -> tuple[list[seaglass.TavernAction], dict[tuple[str, int | None, int | None], int]]:
    actions: list[seaglass.TavernAction] = []
    lookup: dict[tuple[str, int | None, int | None], int] = {}

    def register(act: seaglass.TavernAction) -> None:
        idx = len(actions)
        arg0, arg1 = act.args
        actions.append(act)
        lookup[(act.kind, arg0, arg1)] = idx

    # 1. Global actions (0..4)
    register(seaglass.TavernAction.end_turn())
    register(seaglass.TavernAction.refresh())
    register(seaglass.TavernAction.upgrade_tavern())
    register(seaglass.TavernAction.toggle_freeze())

    # 2. Buy from shop: shop_index in 0..7
    for s_idx in range(MAX_SHOP):
        register(seaglass.TavernAction.buy(s_idx))

    # 3. Play from hand: hand_index in 0..10, board_pos in 0..7
    for h_idx in range(MAX_HAND):
        for b_pos in range(MAX_BOARD):
            register(seaglass.TavernAction.play(h_idx, b_pos))

    # 4. Sell from board: board_pos in 0..7
    for b_pos in range(MAX_BOARD):
        register(seaglass.TavernAction.sell(b_pos))

    # 5. Reposition on board: from_pos != to_pos in 0..7
    for from_pos in range(MAX_BOARD):
        for to_pos in range(MAX_BOARD):
            if from_pos != to_pos:
                register(seaglass.TavernAction.reposition(from_pos, to_pos))

    # 6. Activate board ability: untargeted + targeted (0..10 for board/shop/hand targets)
    for b_pos in range(MAX_BOARD):
        register(seaglass.TavernAction.activate(b_pos, None))
        for t_pos in range(MAX_HAND):
            register(seaglass.TavernAction.activate(b_pos, t_pos))

    # 7. Choose Discover option: option_index in 0..4
    for opt_idx in range(MAX_DISCOVER):
        register(seaglass.TavernAction.choose_discover(opt_idx))

    # 8. Cast a spell from hand on a minion in the Tavern: hand_index in 0..10, shop_index in 0..7
    #    (last, so the indices above are unchanged)
    for h_idx in range(MAX_HAND):
        for s_idx in range(MAX_SHOP):
            register(seaglass.TavernAction.play_on_shop(h_idx, s_idx))

    return actions, lookup


ACTIONS, ACTION_TO_INDEX = _build_action_table()
ACTION_SPACE_SIZE = len(ACTIONS)
END_TURN_INDEX = ACTION_TO_INDEX[("end_turn", None, None)]


def action_from_index(index: int) -> seaglass.TavernAction:
    if not (0 <= index < ACTION_SPACE_SIZE):
        raise ValueError(f"action index {index} out of range 0..{ACTION_SPACE_SIZE}")
    return ACTIONS[index]


def index_from_action(action: seaglass.TavernAction) -> int:
    arg0, arg1 = action.args
    return ACTION_TO_INDEX[(action.kind, arg0, arg1)]


def action_mask(
    game: seaglass.TavernGame,
    allow_reposition: bool = False,
    allow_freeze: bool = False,
    prune_dominated: bool = False,
    obs: seaglass.TavernObservation | None = None,
) -> np.ndarray:
    """Return a boolean mask of shape `(ACTION_SPACE_SIZE,)` for currently legal actions."""
    mask = np.zeros(ACTION_SPACE_SIZE, dtype=np.bool_)
    if prune_dominated:
        if obs is None:
            obs = game.observe()
        board_len = len(obs.board)
        hand_has_minion = any(not u.is_spell for u in obs.hand)
        can_sell = (board_len >= MAX_BOARD and (hand_has_minion or obs.gold >= 2)) or (
            board_len >= 4 and obs.gold == 2
        )
        can_refresh = (obs.free_refreshes > 0) or (obs.gold >= 4)
    else:
        can_sell = True
        can_refresh = True

    for act in game.legal_actions():
        kind = act.kind
        if not allow_reposition and kind == "reposition":
            continue
        if not allow_freeze and kind == "toggle_freeze":
            continue
        if not can_sell and kind == "sell":
            continue
        if not can_refresh and kind == "refresh":
            continue
        arg0, arg1 = act.args
        idx = ACTION_TO_INDEX.get((kind, arg0, arg1))
        if idx is not None:
            mask[idx] = True
    return mask

