"""Baseline opponent bots (`RandomBot` and `HeuristicBot`)."""

from __future__ import annotations

import random
import numpy as np
import seaglass

from .actions import (
    ACTION_TO_INDEX,
    DARK_DISCOVERY_INDEX,
    END_TURN_INDEX,
    action_from_index,
    action_mask,
    index_from_action,
)


def resolve_prompts(game: seaglass.TavernGame, max_steps: int = 64) -> None:
    """Resolve whatever keeps the turn from ending: a pending Discover, a trinket offer (picked
    before anything else), a Choose One, ... (the first option each time)."""
    end = seaglass.TavernAction.end_turn()
    for _ in range(max_steps):
        if game.is_legal(end):
            return
        legal = game.legal_actions()
        if not legal:
            return
        prompt = [a for a in legal if a.kind.startswith("choose_")]
        game.step(prompt[0] if prompt else legal[0])


def finish_turn(game: seaglass.TavernGame) -> None:
    """End the turn, once its pending prompts are resolved."""
    resolve_prompts(game)
    if game.is_legal(seaglass.TavernAction.end_turn()):
        game.end_turn()


def _first_legal_index(game: seaglass.TavernGame) -> int:
    """The index of a legal action that resolves a prompt (End Turn when none is pending)."""
    if game.is_legal(seaglass.TavernAction.end_turn()):
        return END_TURN_INDEX
    for a in game.legal_actions():
        idx = ACTION_TO_INDEX.get((a.kind, *a.args))
        if idx is not None:
            return idx
    return END_TURN_INDEX


class RandomBot:
    """Samples legal actions uniformly (excluding reposition/freeze) with a step cap per turn."""

    def __init__(self, seed: int = 0, end_turn_bias: float = 0.15) -> None:
        self.rng = random.Random(seed)
        self.end_turn_bias = end_turn_bias

    def act(self, game: seaglass.TavernGame) -> int:
        mask = action_mask(game, allow_reposition=False, allow_freeze=False)
        legal = np.flatnonzero(mask)
        if len(legal) == 0:
            return _first_legal_index(game)
        if len(legal) == 1:
            return int(legal[0])
        if mask[END_TURN_INDEX] and self.rng.random() < self.end_turn_bias:
            return END_TURN_INDEX
        non_end = [int(i) for i in legal if i != END_TURN_INDEX]
        if not non_end:
            return END_TURN_INDEX
        return self.rng.choice(non_end)

    def play_turn(self, game: seaglass.TavernGame, max_steps: int = 25) -> None:
        for _ in range(max_steps):
            idx = self.act(game)
            act = action_from_index(idx)
            game.step(act)
            if idx == END_TURN_INDEX:
                return
        finish_turn(game)


class HeuristicBot:
    """Simple rule-based bot:

    0. Picks an offered trinket (the first one it can pay for).
    1. Resolves pending Discovers (picks highest attack + health).
    2. Plays any playable cards from hand (minions & spells), then uses the Dark Discovery button.
    3. Upgrades Tavern on standard turns or when upgrade cost is cheap.
    4. Activates free/cheap board abilities.
    5. Buys the highest-stat minion in the shop if board has space (or buys spells with leftover gold).
    6. Ends the turn.
    """

    def act(self, game: seaglass.TavernGame) -> int:
        obs = game.observe()
        legal_acts = game.legal_actions()

        # 0. A trinket offer comes first (only `choose_trinket` is legal while it is pending)
        trinket_acts = [a for a in legal_acts if a.kind == "choose_trinket"]
        if trinket_acts:
            return index_from_action(trinket_acts[0])

        # 1. Discover pending: pick highest stats/tier
        discover_acts = [a for a in legal_acts if a.kind == "choose_discover"]
        if obs.discover_pending is not None and discover_acts:
            best_act = max(
                discover_acts,
                key=lambda a: (
                    obs.discover_pending[a.args[0]].attack
                    + obs.discover_pending[a.args[0]].health
                    + obs.discover_pending[a.args[0]].tavern_tier * 2
                ),
            )
            return index_from_action(best_act)

        # 2. Play cards from hand (spells or minions when board < 7)
        play_acts = [a for a in legal_acts if a.kind == "play"]
        if play_acts:
            return index_from_action(play_acts[0])

        # 2b. The Dark Discovery button (a minion with a Dark Gift for 3 Gold) with room in hand
        if obs.dark_discovery_usable and len(obs.hand) < 10:
            return DARK_DISCOVERY_INDEX

        # 3. Upgrade Tavern when affordable and on curve (or cost <= 4)
        upgrade_act = seaglass.TavernAction.upgrade_tavern()
        if game.is_legal(upgrade_act):
            if (
                (obs.turn == 2 and obs.tavern_tier == 1)
                or obs.upgrade_cost <= 4
                or (obs.len_board if hasattr(obs, "len_board") else len(obs.board)) >= 6
            ):
                return index_from_action(upgrade_act)

        # 4. Activate abilities
        act_acts = [a for a in legal_acts if a.kind == "activate"]
        if act_acts:
            return index_from_action(act_acts[0])

        # 5. Buy highest-stat minion if board < 7 (or spell with leftover gold)
        buy_acts = [a for a in legal_acts if a.kind == "buy"]
        if buy_acts:
            minion_buys = [a for a in buy_acts if not obs.shop[a.args[0]].is_spell]
            if minion_buys and len(obs.board) < 7:
                best_buy = max(
                    minion_buys,
                    key=lambda a: (
                        obs.shop[a.args[0]].attack
                        + obs.shop[a.args[0]].health
                        + obs.shop[a.args[0]].tavern_tier
                    ),
                )
                return index_from_action(best_buy)
            spell_buys = [a for a in buy_acts if obs.shop[a.args[0]].is_spell]
            if spell_buys and (len(obs.board) >= 1):
                return index_from_action(spell_buys[0])

        # 6. Refresh if we have >= 4 gold and space on board
        refresh_act = seaglass.TavernAction.refresh()
        if obs.gold >= 4 and len(obs.board) < 7 and game.is_legal(refresh_act):
            return index_from_action(refresh_act)

        # Another prompt (a Choose One, ...) still pending: its first option
        return _first_legal_index(game)

    def play_turn(self, game: seaglass.TavernGame, max_steps: int = 25) -> None:
        for _ in range(max_steps):
            idx = self.act(game)
            act = action_from_index(idx)
            game.step(act)
            if idx == END_TURN_INDEX:
                return
        finish_turn(game)


class PolicyBot:
    """Opponent bot driven by a `BattlegroundsMLP` policy network (for self-play)."""

    def __init__(self, model, deterministic: bool = False) -> None:
        self.model = model
        self.deterministic = deterministic

    def act(self, game: seaglass.TavernGame) -> int:
        from .features import encode_observation

        raw = game.observe()
        obs = encode_observation(raw)
        mask = action_mask(game, prune_dominated=True, obs=raw)
        if not mask.any():
            return _first_legal_index(game)
        action, _, _ = self.model.act(obs, mask, deterministic=self.deterministic)
        return action

    def play_turn(self, game: seaglass.TavernGame, max_steps: int = 15) -> None:
        for _ in range(max_steps):
            idx = self.act(game)
            act = action_from_index(idx)
            game.step(act)
            if idx == END_TURN_INDEX:
                return
        finish_turn(game)

