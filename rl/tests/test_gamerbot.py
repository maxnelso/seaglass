"""`9kgamerbot` (`seaglass_rl/gamerbot.py`): plays legal moves to the end, and follows its plan."""

from __future__ import annotations

import seaglass
import pytest

import seaglass_rl.play as play
from seaglass_rl.actions import action_from_index
from seaglass_rl.gamerbot import DIRECTIONS, NineKGamerBot


def test_direction_cards_exist():
    """Every card the directions name is a minion of the pool."""
    ids = {t.card_id for t in seaglass.catalog()}
    for d in DIRECTIONS:
        missing = (d.lynchpins | d.core | d.payoffs) - ids
        assert not missing, f"{d.name}: unknown card ids {sorted(missing)}"


@pytest.mark.parametrize("duos", [False, True])
@pytest.mark.parametrize("name,tempo", [("9kgamerbot", False), ("9kgamerbot-tempo", True)])
def test_plays_legal_moves_and_commits(monkeypatch, duos, name, tempo):
    bots: list[NineKGamerBot] = []
    illegal: list[str] = []
    seen = {"tier": 0}

    class Checked(NineKGamerBot):
        def act(self, game):
            idx = super().act(game)
            if not game.is_legal(action_from_index(idx)):
                illegal.append(repr(action_from_index(idx)))
            seen["tier"] = max(seen["tier"], game.observe().tavern_tier)
            return idx

    orig = play._bot

    def make(controller, seed):
        if controller == name:
            bot = Checked(tempo_levels=tempo)
            bots.append(bot)
            return bot
        return orig(controller, seed)

    monkeypatch.setattr(play, "_bot", make)
    config = play.LobbyConfig(duos=duos, tribes=5)
    half = config.units // 2
    specs = [play.GameSpec(seed=40 + g, controllers=[name] * half + ["heuristic"] * (config.units - half))
             for g in range(3)]
    results = play.play_games(config, specs, {}, play.PlayOptions())
    assert all(r.placements for r in results)
    assert not illegal, illegal[:5]
    # Most seats commit to a direction, and the bot gets past Tier 4.
    assert sum(b.direction is not None for b in bots) >= len(bots) // 2
    assert seen["tier"] >= 5
