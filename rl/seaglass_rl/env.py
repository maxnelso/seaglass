"""Gym-like head-to-head Battlegrounds environment wrapping `seaglass.TavernGame`."""

from __future__ import annotations

from typing import Any
import numpy as np
import seaglass

from .actions import END_TURN_INDEX, action_from_index, action_mask
from .bots import HeuristicBot, PolicyBot, RandomBot, finish_turn
from .features import encode_observation


def compute_board_power(obs: seaglass.TavernObservation) -> float:
    b_stat = (
        sum(
            u.attack
            + u.health
            + (3 if u.divine_shield else 0)
            + (2 if (u.reborn or u.windfury or u.venomous) else 0)
            for u in obs.board
        )
        / 50.0
    )
    h_stat = sum((0.02 if u.is_spell else (u.attack + u.health) / 200.0) for u in obs.hand)
    return b_stat + h_stat + (obs.tavern_tier - 1) * 0.04


class BattlegroundsEnv:
    """Single-agent RL environment that plays full Battlegrounds matches against an opponent bot."""

    def __init__(
        self,
        opponent: str = "random",
        catalog: str = "full",
        max_turns: int = 20,
        max_steps_per_turn: int = 15,
        seed: int = 42,
        model=None,
    ) -> None:
        self.opponent_kind = opponent
        self.catalog = catalog
        self.max_turns = max_turns
        self.max_steps_per_turn = max_steps_per_turn
        self.base_seed = seed
        self.model = model
        self.episode_count = 0

        self.player: seaglass.TavernGame | None = None
        self.opp_game: seaglass.TavernGame | None = None
        # What the two players know about each other (lobby Hero Powers: Scabbs, Tess, Holmes,
        # Mukla, ...). Never part of the observation.
        self.lobby = seaglass.Lobby(2)
        self.opp_bot: RandomBot | HeuristicBot | PolicyBot = self._make_opponent(seed)
        self.steps_this_turn = 0

    def _make_opponent(self, seed: int) -> RandomBot | HeuristicBot | PolicyBot:
        if self.opponent_kind == "heuristic":
            return HeuristicBot()
        if self.opponent_kind == "self" and self.model is not None:
            return PolicyBot(self.model)
        return RandomBot(seed=seed + 999)

    def _board_power(self, obs: seaglass.TavernObservation) -> float:
        return compute_board_power(obs)

    def _start_turns(self) -> None:
        """Both players start their turn, each the other's next opponent (head to head: no featured
        pair to wager on), and their lobby gifts reach each other."""
        assert self.player is not None and self.opp_game is not None
        games = [self.player, self.opp_game]
        self.lobby.set_next_pairs([(0, 1)])
        self.lobby.apply(games)
        self.player.start_turn()
        self.opp_game.start_turn()
        self.lobby.deliver_gifts(games)

    def reset(self, seed: int | None = None) -> tuple[tuple[np.ndarray, np.ndarray, np.ndarray], np.ndarray]:
        if seed is None:
            seed = self.base_seed + self.episode_count * 2
        self.episode_count += 1

        # Every game has trinkets and Dark Gifts (`engine/docs/trinkets.md`, `dark_gifts.md`).
        self.player = seaglass.TavernGame(
            seed=seed, catalog=self.catalog, deity="cthun", trinkets=True, dark_gifts=True
        )
        self.opp_game = seaglass.TavernGame(
            seed=seed + 1, catalog=self.catalog, deity="yshaarj", trinkets=True, dark_gifts=True
        )
        self.opp_bot = self._make_opponent(seed)
        self.lobby = seaglass.Lobby(2)

        self._start_turns()
        self.steps_this_turn = 0

        obs = self.player.observe()
        return encode_observation(obs), action_mask(self.player, prune_dominated=True, obs=obs)

    def step(
        self, action_idx: int
    ) -> tuple[tuple[np.ndarray, np.ndarray, np.ndarray], float, bool, np.ndarray, dict[str, Any]]:
        assert self.player is not None and self.opp_game is not None

        pre_obs = self.player.observe()
        pre_power = self._board_power(pre_obs)

        act = action_from_index(action_idx)
        if not self.player.is_legal(act):
            raise ValueError(f"Illegal action {act} (index {action_idx}) on turn {pre_obs.turn}")

        self.player.step(act)
        self.steps_this_turn += 1

        # Force EndTurn if the agent hits the per-turn micro-step cap (pending prompts resolved
        # first: a Discover, a trinket offer, ...)
        turn_ended = action_idx == END_TURN_INDEX
        if not turn_ended and self.steps_this_turn >= self.max_steps_per_turn:
            finish_turn(self.player)
            turn_ended = True

        reward = self._board_power(self.player.observe()) - pre_power
        done = False
        info: dict[str, Any] = {}

        if not turn_ended:
            post_obs = self.player.observe()
            return (
                encode_observation(post_obs),
                float(reward),
                False,
                action_mask(self.player, prune_dominated=True, obs=post_obs),
                info,
            )

        # Opponent takes its Tavern turn, then both boards fight
        self.opp_bot.play_turn(self.opp_game, max_steps=self.max_steps_per_turn)
        self.lobby.deliver_gifts([self.player, self.opp_game])
        pre_combat_agent = self.player.observe()
        pre_combat_opp = self.opp_game.observe()
        battle = self.player.resolve_combat_with(self.opp_game)
        self.lobby.record_combat(0, 1, pre_combat_agent.board)
        self.lobby.record_combat(1, 0, pre_combat_opp.board)
        self.lobby.record_outcome(0, 1, battle.outcome)

        if battle.outcome == "a_win":
            reward += 0.2 + battle.hero_damage / 30.0
        elif battle.outcome == "b_win":
            reward -= 0.2 + battle.hero_damage / 30.0

        p_obs = self.player.observe()
        o_obs = self.opp_game.observe()
        self.lobby.record_deaths([p_obs.health > 0, o_obs.health > 0])
        info["combat"] = {
            "turn": pre_combat_agent.turn,
            "agent_board": pre_combat_agent.board,
            "opp_board": pre_combat_opp.board,
            "opp_tier": pre_combat_opp.tavern_tier,
            "battle": battle,
            "post_agent": p_obs,
            "post_opp": o_obs,
        }

        if p_obs.health <= 0 or o_obs.health <= 0 or p_obs.turn >= self.max_turns:
            done = True
            if p_obs.health > o_obs.health and o_obs.health <= 0:
                reward += 2.0
                info["winner"] = "agent"
            elif o_obs.health > p_obs.health and p_obs.health <= 0:
                reward -= 2.0
                info["winner"] = "opponent"
            elif p_obs.health > o_obs.health:
                reward += 1.0
                info["winner"] = "agent"
            elif o_obs.health > p_obs.health:
                reward -= 1.0
                info["winner"] = "opponent"
            else:
                info["winner"] = "draw"
            info["turns"] = p_obs.turn
            info["agent_hp"] = p_obs.health
            info["opp_hp"] = o_obs.health
            return (
                encode_observation(p_obs),
                float(reward),
                True,
                np.zeros_like(action_mask(self.player)),
                info,
            )

        # Advance to the next turn
        self._start_turns()
        self.steps_this_turn = 0
        next_obs = self.player.observe()
        return (
            encode_observation(next_obs),
            float(reward),
            False,
            action_mask(self.player, prune_dominated=True, obs=next_obs),
            info,
        )

