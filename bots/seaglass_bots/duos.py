"""Sequential Multi-Agent Rollout Bot for Duos (`DuosTavern` + `Pass` coordination)."""

from __future__ import annotations

import random
from typing import Dict, Optional, Sequence

import seaglass

from .catalog_meta import get_meta_db
from .search import ShopEpochBeamBot, choose_best_hero_action


class DuosSequentialRolloutBot:
    """
    Coordinates a Duos team via Bertsekas Sequential Multi-Agent Rollout:
      1. Evaluates marginal team utility of `TavernAction.pass_card` when a hand/shop minion
         completes a teammate's Triple/Pair or feeds their dominant carry tribe.
      2. Runs full multi-step `ShopEpochBeamBot` (DAG Beam Search + ICM + PositioningSolver)
         on each player's belief tavern, verified with joint `DuosTavernGame.equity_against`.
    """

    def __init__(self, catalog_name: str = "duos", seed: int = 777) -> None:
        self.meta = get_meta_db(catalog_name)
        self.solo_planner = ShopEpochBeamBot(catalog_name=catalog_name, seed=seed)
        self.rng = random.Random(seed)
        self.last_note: Optional[Dict[str, float]] = None

    @staticmethod
    def _dominant_tribe(units: Sequence[seaglass.Unit]) -> Optional[str]:
        counts: Dict[str, int] = {}
        for u in units:
            for tr in u.tribes:
                counts[tr] = counts.get(tr, 0) + 1
        if not counts:
            return None
        best_tr, cnt = max(counts.items(), key=lambda kv: kv[1])
        return best_tr if cnt >= 2 else None

    def act_in_lobby(
        self, lobby: seaglass.LobbyGame, player: int
    ) -> seaglass.TavernAction:
        legal = lobby.legal_actions(player)
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            return legal[0]

        obs = lobby.observe(player)
        if lobby.phase == "hero" or legal[0].kind == "choose_hero":
            return choose_best_hero_action(obs, legal, self.meta)

        # Inspect teammate's board + hand for Triple / Carry-Tribe Pass opportunities!
        mate_units = (
            list(obs.teammate_board)
            + list(obs.teammate_hand)
            + list(obs.teammate_incoming)
        )
        mate_counts: Dict[int, int] = {}
        for u in mate_units:
            if not u.is_golden and not u.is_spell:
                mate_counts[u.card_id] = mate_counts.get(u.card_id, 0) + 1

        own_counts: Dict[int, int] = {}
        for u in list(obs.board) + list(obs.hand):
            if not u.is_golden and not u.is_spell:
                own_counts[u.card_id] = own_counts.get(u.card_id, 0) + 1

        mate_tribe = self._dominant_tribe(obs.teammate_board)
        own_tribe = self._dominant_tribe(obs.board)

        # 1. Pass hand card if it gives the teammate a Triple or uses a free Pass (`Wanderer Cho`)
        #    for a high-tier card of the teammate's carry tribe.
        if legal[0].kind not in ("choose_discover", "choose_trinket"):
            for act in legal:
                if act.kind == "pass":
                    h_idx, _ = act.args
                    if h_idx is not None and h_idx < len(obs.hand):
                        card = obs.hand[h_idx]
                        completes_mate_triple = (
                            not card.is_spell
                            and not card.is_golden
                            and mate_counts.get(card.card_id, 0) >= 2
                            and own_counts.get(card.card_id, 0) < 3
                        )
                        free_tribe_gift = (
                            obs.free_passes > 0
                            and mate_tribe is not None
                            and mate_tribe != own_tribe
                            and mate_tribe in card.tribes
                            and card.tavern_tier >= max(2, obs.teammate_tavern_tier)
                        )
                        if completes_mate_triple or free_tribe_gift:
                            # Clear queued single-player plan since we mutated hand via Pass.
                            self.solo_planner._queued_actions.pop(player, None)
                            self.last_note = {"duos_pass_triple": 1.0 if completes_mate_triple else 0.5}
                            return act

            # 2. Buy a shop minion specifically to Pass if it completes the teammate's Triple!
            pass_cost = 0 if obs.free_passes > 0 else 1
            if obs.gold >= obs.minion_cost + pass_cost and len(obs.hand) < 10:
                for act in legal:
                    if act.kind == "buy":
                        s_idx, _ = act.args
                        if s_idx is not None and s_idx < len(obs.shop):
                            sc = obs.shop[s_idx]
                            if (
                                not sc.is_spell
                                and not sc.is_golden
                                and mate_counts.get(sc.card_id, 0) >= 2
                                and own_counts.get(sc.card_id, 0) == 0
                            ):
                                self.solo_planner._queued_actions.pop(player, None)
                                self.last_note = {"duos_buy_for_pass": 1.0}
                                return act

        # 3. Sequential Rollout: solve this player's full tavern turn with Shop-Epoch DAG Beam Search
        #    + ICM Survival + PositioningSolver on the belief-depleted Duos pool!
        ctx = lobby.context(player)
        depletion = self.solo_planner.belief.sample_pool_depletion_units(
            lobby.turn, ctx
        )
        belief_game = lobby.belief_tavern(
            player, seed=self.rng.getrandbits(63) | 1, extra_units=depletion
        )
        act = self.solo_planner.choose_action_for_tavern(
            belief_game, legal, player=player, ctx=ctx
        )
        self.last_note = self.solo_planner.last_note
        return act
