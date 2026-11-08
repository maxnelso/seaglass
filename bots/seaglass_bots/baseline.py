"""League Baseline Bots: SemiRandomBot (matching tools/src/bot.rs), TempoBot, and GreedyLevelBot."""

from __future__ import annotations

import random
from typing import List, Sequence

import seaglass


class SemiRandomBot:
    """Faithful Python counterpart to `choose_semi_random_action` in `tools/src/bot.rs`."""

    def __init__(self, seed: int = 101) -> None:
        self.rng = random.Random(seed)

    def act_in_lobby(
        self, lobby: seaglass.LobbyGame, player: int
    ) -> seaglass.TavernAction:
        legal = lobby.legal_actions(player)
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1 or lobby.phase == "hero":
            return self.rng.choice(legal)

        obs = lobby.observe(player)

        # 1. Resolve pending prompts immediately.
        prompts = [
            a
            for a in legal
            if a.kind in ("choose_discover", "choose_trinket", "choose_hero")
        ]
        if prompts:
            return self.rng.choice(prompts)

        # 2. Play playable spells and minions from hand.
        play_acts = [a for a in legal if a.kind == "play"]
        if play_acts:
            # Prefer playing spells first, then minions at rightmost slot or magnetic fuse.
            spell_plays = [
                a
                for a in play_acts
                if a.args[0] is not None
                and a.args[0] < len(obs.hand)
                and obs.hand[a.args[0]].is_spell
            ]
            if spell_plays:
                return self.rng.choice(spell_plays)
            return self.rng.choice(play_acts)

        # 3. Sell weakest non-golden board minion if board is full and hand holds a stronger minion.
        if len(obs.board) >= 7:
            playable_minions = [
                c for c in obs.hand if not c.is_spell and not c.unplayable
            ]
            if playable_minions:
                best_h = max(
                    playable_minions,
                    key=lambda c: (c.is_golden, c.tavern_tier, c.attack + c.health),
                )
                non_golden = [
                    (i, b) for i, b in enumerate(obs.board) if not b.is_golden
                ]
                if non_golden:
                    b_pos, weak_b = min(
                        non_golden, key=lambda ib: (ib[1].attack + ib[1].health, ib[1].tavern_tier)
                    )
                    h_score = (
                        best_h.attack
                        + best_h.health
                        + 3 * best_h.tavern_tier
                        + (10 if best_h.is_golden else 0)
                    )
                    b_score = weak_b.attack + weak_b.health + 2 * weak_b.tavern_tier
                    if h_score > b_score:
                        for a in legal:
                            if a.kind == "sell" and a.args[0] == b_pos:
                                return a

        # 4. Activate abilities 70% of the time.
        activates = [a for a in legal if a.kind == "activate"]
        if activates and self.rng.randrange(10) < 7:
            return self.rng.choice(activates)

        # 5. Upgrade Tavern Tier heuristically.
        upgrades = [a for a in legal if a.kind == "upgrade_tavern"]
        if upgrades:
            should_up = (
                obs.upgrade_cost <= 4
                or (len(obs.board) >= 4 and self.rng.randrange(10) < 6)
                or self.rng.randrange(10) < 3
            )
            if should_up:
                return upgrades[0]

        # 6. Buy minions/spells (preferring pairs/triples).
        buys = [a for a in legal if a.kind == "buy"]
        if buys:
            owned_ids = {
                u.card_id
                for u in list(obs.board) + list(obs.hand)
                if not u.is_golden and not u.is_spell
            }
            pair_buys = [
                a
                for a in buys
                if a.args[0] is not None
                and a.args[0] < len(obs.shop)
                and not obs.shop[a.args[0]].is_spell
                and obs.shop[a.args[0]].card_id in owned_ids
            ]
            if pair_buys:
                return self.rng.choice(pair_buys)
            filtered_buys = [
                a
                for a in buys
                if a.args[0] is not None
                and a.args[0] < len(obs.shop)
                and not (
                    not obs.shop[a.args[0]].is_spell
                    and len(obs.board) >= 7
                    and len(obs.hand) >= 3
                )
            ]
            if filtered_buys:
                return self.rng.choice(filtered_buys)

        # 7. Refresh if >= 4 gold or free refreshes.
        refreshes = [a for a in legal if a.kind == "refresh"]
        if refreshes and (
            obs.free_refreshes > 0
            or obs.gold >= 4
            or (obs.gold >= 1 and not obs.shop)
        ):
            return refreshes[0]

        end_acts = [a for a in legal if a.kind == "end_turn"]
        return end_acts[0] if end_acts else legal[0]


class TempoBot:
    """Aggressive roach/tempo league archetype: prioritizes immediate stats on board over leveling."""

    def __init__(self, seed: int = 202) -> None:
        self.rng = random.Random(seed)
        self.fallback = SemiRandomBot(seed=seed + 1)

    def act_in_lobby(
        self, lobby: seaglass.LobbyGame, player: int
    ) -> seaglass.TavernAction:
        legal = lobby.legal_actions(player)
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1 or lobby.phase == "hero":
            return self.rng.choice(legal)

        obs = lobby.observe(player)
        prompts = [
            a
            for a in legal
            if a.kind in ("choose_discover", "choose_trinket", "choose_hero")
        ]
        if prompts:
            return prompts[0]

        # Play highest-stat cards immediately.
        plays = [a for a in legal if a.kind == "play"]
        if plays:
            return plays[-1]

        # Only upgrade when cheap (cost <= 3) or board already has 6+ minions.
        upgrades = [a for a in legal if a.kind == "upgrade_tavern"]
        if upgrades and (obs.upgrade_cost <= 3 or (len(obs.board) >= 6 and obs.upgrade_cost <= 6)):
            return upgrades[0]

        # Buy highest attack+health minion in shop!
        buys = [
            a
            for a in legal
            if a.kind == "buy"
            and a.args[0] is not None
            and a.args[0] < len(obs.shop)
        ]
        if buys and (len(obs.board) < 7 or len(obs.hand) < 2):
            best_buy = max(
                buys,
                key=lambda a: (
                    obs.shop[a.args[0]].attack
                    + obs.shop[a.args[0]].health
                    + (4 if obs.shop[a.args[0]].divine_shield else 0)
                ),
            )
            if len(obs.board) < 7 or not obs.shop[best_buy.args[0]].is_spell:
                return self.fallback.act_in_lobby(lobby, player) if len(obs.board) >= 7 else best_buy

        HeroP = [a for a in legal if a.kind == "hero_power" and obs.hero_power_cost <= 1]
        if HeroP:
            return HeroP[0]

        return self.fallback.act_in_lobby(lobby, player)


class GreedyLevelBot:
    """Power-leveling ("3-on-3" / fast Tier 5) league archetype."""

    def __init__(self, seed: int = 303) -> None:
        self.rng = random.Random(seed)
        self.fallback = SemiRandomBot(seed=seed + 1)

    def act_in_lobby(
        self, lobby: seaglass.LobbyGame, player: int
    ) -> seaglass.TavernAction:
        legal = lobby.legal_actions(player)
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1 or lobby.phase == "hero":
            return self.rng.choice(legal)

        obs = lobby.observe(player)
        # Always upgrade up to Tier 5 whenever healthy (HP+Armor >= 18)!
        upgrades = [a for a in legal if a.kind == "upgrade_tavern"]
        if upgrades and obs.tavern_tier < 5 and (obs.health + obs.armor >= 18):
            return upgrades[0]

        return self.fallback.act_in_lobby(lobby, player)
