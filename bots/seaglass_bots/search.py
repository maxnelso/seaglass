"""Intra-Turn Search Bots: Shop-Epoch DAG Beam Search (with Cycle Macros & Batched Neural Eval) & Expectimax MCTS."""

from __future__ import annotations

import math
import os
import random
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Sequence, Tuple

import seaglass

from .belief import LobbyBeliefFilter, OpponentParticleSet
from .catalog_meta import CardMetaDB, get_meta_db
from .evaluator import LeafEvaluator
from .positioning import PositioningSolver

ActionKey = Tuple[str, Optional[int], Optional[int]]
# Queued step: (action_key, expected_plan_validation_key, stop_queue_after_step)
QueuedStep = Tuple[ActionKey, Tuple, bool]


def _action_key(a: seaglass.TavernAction) -> ActionKey:
    arg0, arg1 = a.args
    return (a.kind, arg0, arg1)


def _plan_validation_key(obs: seaglass.TavernObservation) -> Tuple:
    """
    Exact state invariant for queued-plan execution:
      - Order-invariant across board `Reposition` swaps,
      - Immediately invalidates stale queued plans if ANY shop card, hand card, board stat,
        gold, tier, or Discover prompt diverges from the simulated deterministic path!
    """
    board_multiset = tuple(
        sorted((u.card_id, u.attack, u.health, u.is_golden) for u in obs.board)
    )
    hand_exact = tuple(
        (c.card_id, c.is_golden, c.is_spell, c.unplayable) for c in obs.hand
    )
    shop_exact = tuple(
        (s.card_id, s.is_spell, s.spell_cost) for s in obs.shop
    )
    return (
        obs.gold,
        obs.tavern_tier,
        obs.upgrade_cost,
        obs.free_refreshes,
        obs.hero_power_uses_left,
        obs.discover_pending,
        board_multiset,
        hand_exact,
        shop_exact,
    )


def _decision_skeleton(obs: seaglass.TavernObservation) -> Tuple:
    """Card-ID and gold skeleton: differs across seeds iff an action rolls random cards or gold."""
    return (
        obs.gold,
        obs.tavern_tier,
        obs.upgrade_cost,
        obs.free_refreshes,
        obs.discover_pending,
        tuple(sorted(u.card_id for u in obs.board)),
        tuple(sorted(c.card_id for c in obs.hand)),
        tuple(sorted(s.card_id for s in obs.shop)),
    )


def _canonical_state_key(obs: seaglass.TavernObservation) -> Tuple:
    """Order-invariant shop/hand + board signature for DAG transposition deduplication."""
    board_sig = tuple(
        (
            u.card_id,
            u.attack,
            u.health,
            u.is_golden,
            u.taunt,
            u.divine_shield,
            u.venomous,
            u.reborn,
        )
        for u in obs.board
    )
    hand_sig = tuple(
        sorted(
            (c.card_id, c.attack, c.health, c.is_golden, c.is_spell, c.unplayable)
            for c in obs.hand
        )
    )
    shop_sig = tuple(
        sorted(
            (s.card_id, s.attack, s.health, s.is_spell, s.frozen)
            for s in obs.shop
        )
    )
    return (
        obs.gold,
        obs.tavern_tier,
        obs.upgrade_cost,
        obs.is_frozen,
        obs.free_refreshes,
        obs.hero_power_uses_left,
        obs.second_hero_power_uses_left,
        obs.dark_discoveries_left,
        board_sig,
        hand_sig,
        shop_sig,
        tuple(obs.player_counters),
    )


def expendable_board_indices(
    obs: seaglass.TavernObservation,
    meta: CardMetaDB,
    max_slots: int = 2,
) -> List[int]:
    """Identify the weakest / cycle-slot board indices eligible for selling."""
    n = len(obs.board)
    if n == 0:
        return []
    if n <= max_slots:
        return list(range(n))

    ranked: List[Tuple[float, int]] = []
    for idx, u in enumerate(obs.board):
        prof = meta.profile(u.card_id)
        keep_score = (
            0.5 * u.attack
            + 0.5 * u.health
            + 1.5 * u.tavern_tier
            + (6.0 if u.is_golden else 0.0)
            + (5.0 if u.venomous else 0.0)
            + (2.5 if u.divine_shield else 0.0)
        )
        if prof is not None:
            keep_score += 1.4 * prof.engine_weight
            # Genuine on-sell economy units (`Sellemental`, `Patient Scout`, etc.) are meant to be sold!
            if prof.has_on_sell and not prof.is_scaling_engine:
                keep_score -= 6.0
        ranked.append((keep_score, idx))

    ranked.sort(key=lambda x: x[0])
    return [idx for _, idx in ranked[:max_slots]]


def prune_tavern_actions(
    obs: seaglass.TavernObservation,
    legal: Sequence[seaglass.TavernAction],
    meta: CardMetaDB,
    legacy_mode: bool = False,
) -> List[seaglass.TavernAction]:
    """Prune redundant Reposition, symmetric Play slots, bad early/big Sells, and full-board junk Buys."""
    out: List[seaglass.TavernAction] = []
    seen_play_minion_hand: Dict[int, bool] = {}
    weak_slots = expendable_board_indices(obs, meta, max_slots=2)
    sell_allowed = set(weak_slots)

    # Compute weakest slot stats and owned copy counts for full-board & early-game discipline.
    cycle_stat_cap = max(28, int(2.6 * obs.turn))
    weakest_idx = weak_slots[0] if weak_slots else -1
    weakest_stats = (
        (obs.board[weakest_idx].attack + obs.board[weakest_idx].health)
        if weakest_idx >= 0
        else 0
    )
    weakest_prof = (
        meta.profile(obs.board[weakest_idx].card_id) if weakest_idx >= 0 else None
    )
    has_cycle_slot = weakest_idx >= 0 and (
        weakest_stats < cycle_stat_cap
        or (weakest_prof is not None and weakest_prof.has_on_sell)
    )
    has_mech_target = any(
        ("mech" in u.tribes or "all" in u.tribes) for u in obs.board
    )
    owned_copies: Dict[int, int] = {}
    for u in list(obs.board) + list(obs.hand):
        if not u.is_golden and not u.is_spell:
            owned_copies[u.card_id] = owned_copies.get(u.card_id, 0) + 1

    for act in legal:
        k = act.kind
        if k == "reposition":
            continue
        if k == "sell":
            b_pos, _ = act.args
            if b_pos is None or b_pos >= len(obs.board):
                continue
            if legacy_mode:
                if len(obs.board) > 3 and b_pos not in sell_allowed:
                    continue
                out.append(act)
                continue

            u_sell = obs.board[b_pos]
            u_prof = meta.profile(u_sell.card_id)
            has_on_sell = u_prof.has_on_sell if u_prof is not None else False
            u_stats = u_sell.attack + u_sell.health

            if len(obs.board) < 7:
                # Never sell board minions into empty space unless:
                #   (a) genuine `on_sell` minion (`Sellemental`, `Patient Scout`), OR
                #   (b) Turn >= 3, weakest small non-engine minion (`<= 10` stats), AND `+1g`
                #       hits an exact breakpoint (`gold + 1 == minion_cost` or `upgrade_cost`)!
                if has_on_sell:
                    out.append(act)
                elif (
                    obs.turn >= 3
                    and b_pos == weakest_idx
                    and not u_sell.is_golden
                    and not (u_prof is not None and u_prof.is_scaling_engine)
                    and u_stats <= 10
                    and (
                        obs.gold + 1 == obs.minion_cost
                        or obs.gold + 1 == obs.upgrade_cost
                    )
                ):
                    out.append(act)
                continue

            # Full board (`len == 7`): only allow raw `Sell` on an actual small cycle slot or `on_sell` unit!
            if b_pos in sell_allowed and (has_on_sell or u_stats < cycle_stat_cap):
                out.append(act)
            continue

        if k == "buy" and not legacy_mode and len(obs.board) >= 7:
            s_idx, _ = act.args
            if s_idx is not None and s_idx < len(obs.shop):
                s_card = obs.shop[s_idx]
                if not s_card.is_spell:
                    is_triple = (
                        not s_card.is_golden
                        and owned_copies.get(s_card.card_id, 0) >= 2
                    )
                    is_mag = s_card.magnetic and has_mech_target
                    s_prof = meta.profile(s_card.card_id)
                    s_stats = s_card.attack + s_card.health
                    is_big_upgrade = s_stats >= 0.65 * weakest_stats or (
                        s_prof is not None
                        and s_prof.is_scaling_engine
                        and s_card.tavern_tier >= 5
                        and weakest_stats < 55
                    )
                    if not (is_triple or is_mag or has_cycle_slot or is_big_upgrade):
                        continue

        if k == "play":
            h_idx, b_pos = act.args
            if h_idx is not None and h_idx < len(obs.hand):
                card = obs.hand[h_idx]
                if not card.is_spell:
                    prof = meta.profile(card.card_id)
                    is_mag_target = (
                        card.magnetic
                        and b_pos is not None
                        and b_pos < len(obs.board)
                    )
                    has_bc = prof.has_battlecry if prof else False
                    if is_mag_target or (has_bc and b_pos in (0, len(obs.board))):
                        out.append(act)
                        seen_play_minion_hand[h_idx] = True
                        continue
                    if not seen_play_minion_hand.get(h_idx, False) and b_pos == len(obs.board):
                        out.append(act)
                        seen_play_minion_hand[h_idx] = True
                        continue
                    continue
        out.append(act)

    if not out and legal:
        return list(legal)
    return out


def choose_best_hero_action(
    obs: seaglass.TavernObservation,
    legal: Sequence[seaglass.TavernAction],
    meta: CardMetaDB,
    base_game: Optional[seaglass.TavernGame] = None,
    evaluator: Optional[LeafEvaluator] = None,
) -> seaglass.TavernAction:
    """Pick offered hero using starting HP/Armor + Hero Power hooks + neural value."""
    hero_info = {hid: (name, impl) for hid, name, impl in seaglass.heroes_table()}
    best_act = legal[0]
    best_score = -1e9
    for act in legal:
        if act.kind != "choose_hero":
            continue
        opt_idx, _ = act.args
        if opt_idx is None or opt_idx >= len(obs.hero_options):
            continue
        hid = obs.hero_options[opt_idx]
        prof = meta.profile(hid)
        score = 0.0
        if hid in hero_info and hero_info[hid][1]:
            score += 2.5
        if prof is not None:
            score += prof.engine_weight
            if prof.has_start_of_combat:
                score += 2.5
            if prof.is_economy_generator:
                score += 2.2
        if base_game is not None:
            sim = base_game.copy()
            if sim.is_legal(act):
                sim.step(act)
                s_obs = sim.observe()
                score += 0.28 * (s_obs.health + s_obs.armor - 30)
                if evaluator is not None and evaluator.has_neural_model:
                    score += 0.5 * evaluator.evaluate_fast(s_obs, end_of_turn=True)
        if score > best_score:
            best_score = score
            best_act = act
    return best_act


class ShopEpochBeamBot:
    """
    Plans each turn with:
      1. LobbyBeliefFilter (Sequential Monte Carlo opponent & pool particles),
      2. Shop-Epoch DAG Beam Search with batched Set-Transformer pruning + full-board Cycle Macros,
      3. 1-Turn Rust Physics Rollouts (`end_turn` -> `resolve_combat_against` -> `start_turn`),
      4. Successive Halving + 1v1 Regret-Matching PositioningSolver at EndTurn.
    """

    def __init__(
        self,
        catalog_name: str = "full",
        beam_width: int = 12,
        max_epoch_depth: int = 8,
        top_k_combat_evals: int = 4,
        refresh_samples: int = 3,
        num_particles: int = 3,
        seed: int = 42,
        evaluator: Optional[LeafEvaluator] = None,
        checkpoint_path: Optional[os.PathLike | str] = None,
        legacy_mode: bool = False,
    ) -> None:
        self.catalog_name = catalog_name
        self.meta = get_meta_db(catalog_name)
        self.belief = LobbyBeliefFilter(
            catalog_name=catalog_name, num_particles=num_particles, seed=seed
        )
        self.evaluator = (
            evaluator
            if evaluator is not None
            else LeafEvaluator(
                catalog_name=catalog_name,
                checkpoint_path=checkpoint_path,
                legacy_mode=legacy_mode,
            )
        )
        self.positioner = PositioningSolver(
            catalog_name=catalog_name,
            max_candidates=48,
            seed=seed + 7,
            evaluator=self.evaluator,
        )
        self.beam_width = beam_width
        self.max_epoch_depth = max_epoch_depth
        self.top_k_combat_evals = top_k_combat_evals
        self.refresh_samples = refresh_samples
        self.legacy_mode = legacy_mode
        self.rng = random.Random(seed)
        self._queued_actions: Dict[int, List[QueuedStep]] = {}
        self._positioned_turn: Dict[int, int] = {}
        self.last_note: Optional[Dict[str, float]] = None

    def _find_matching_legal(
        self,
        legal: Sequence[seaglass.TavernAction],
        target_key: ActionKey,
    ) -> Optional[seaglass.TavernAction]:
        for act in legal:
            if _action_key(act) == target_key:
                return act
        return None

    def _evaluate_prompt_options(
        self,
        base_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        particles: OpponentParticleSet,
        ctx: Optional[seaglass.LobbyContext],
    ) -> seaglass.TavernAction:
        best_act = legal[0]
        best_val = -1e18
        for act in legal:
            sim = base_game.copy()
            sim.reseed(self.rng.getrandbits(63) | 1)
            sim.step(act)
            obs_after = sim.observe()
            # Play newly discovered card onto open slot OR cycle weakest slot if clearly stronger!
            if obs_after.discover_pending is None and not obs_after.trinket_options and obs_after.hand:
                new_idx = len(obs_after.hand) - 1
                new_card = obs_after.hand[new_idx]
                if not new_card.unplayable:
                    if new_card.is_spell:
                        for sub in sim.legal_actions():
                            if sub.kind == "play" and sub.args[0] == new_idx:
                                sim.step(sub)
                                break
                    elif len(obs_after.board) < 7:
                        play_act = seaglass.TavernAction.play(new_idx, len(obs_after.board))
                        if sim.is_legal(play_act):
                            sim.step(play_act)
                    elif not self.legacy_mode and len(obs_after.board) == 7:
                        weak_slots = expendable_board_indices(obs_after, self.meta, max_slots=1)
                        if weak_slots:
                            w_slot = weak_slots[0]
                            w_unit = obs_after.board[w_slot]
                            if (
                                new_card.is_golden
                                or new_card.tavern_tier > w_unit.tavern_tier
                                or (new_card.attack + new_card.health) > (w_unit.attack + w_unit.health + 4)
                            ):
                                sell_act = seaglass.TavernAction.sell(w_slot)
                                if sim.is_legal(sell_act):
                                    sim.step(sell_act)
                                    # Re-locate card at end of hand and play it.
                                    obs_mid = sim.observe()
                                    if obs_mid.hand:
                                        p_act = seaglass.TavernAction.play(
                                            len(obs_mid.hand) - 1, min(w_slot, len(obs_mid.board))
                                        )
                                        if sim.is_legal(p_act):
                                            sim.step(p_act)

            val = self.evaluator.evaluate_turn_end(
                sim, particles, ctx=ctx, combats_per_particle=8
            )
            if val > best_val:
                best_val = val
                best_act = act
        return best_act

    def _is_stochastic_or_shop_hero_power(
        self,
        g: seaglass.TavernGame,
        obs: seaglass.TavernObservation,
        act: seaglass.TavernAction,
    ) -> bool:
        """
        Detect Hero Powers that mutate the shop (`Bob's Burgles`, `Temporal Tavern`) or roll
        stochastic outcomes across RNG seeds so they are routed through `_evaluate_stochastic_action`
        instead of hallucinating a single lucky RNG roll inside deterministic `_search_shop_epoch`.
        """
        if self.legacy_mode or act.kind != "hero_power":
            return False
        s1 = g.copy()
        s1.reseed(100003)
        s1.step(act)
        o1 = s1.observe()
        if o1.discover_pending is not None:
            return False
        if [x.card_id for x in o1.shop] != [x.card_id for x in obs.shop]:
            return True
        s2 = g.copy()
        s2.reseed(200003)
        s2.step(act)
        o2 = s2.observe()
        return _decision_skeleton(o1) != _decision_skeleton(o2)

    def _expand_cycle_macros(
        self,
        g: seaglass.TavernGame,
        obs: seaglass.TavernObservation,
        path: List[QueuedStep],
    ) -> List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]]:
        """
        When board has 7 units and hand holds playable minions (Goldens, Battlecries, upgrades),
        generate compound `Sell(weak_slot) -> Play(hand_idx)` transitions — ONLY when `weak_slot`
        is an actual cycle slot or the replacement minion genuinely justifies replacing `weak_slot`!
        """
        if self.legacy_mode or len(obs.board) < 7 or not obs.hand:
            return []
        weak_slots = expendable_board_indices(obs, self.meta, max_slots=1)
        if not weak_slots:
            return []
        w_slot = weak_slots[0]
        w_unit = obs.board[w_slot]
        w_stats = w_unit.attack + w_unit.health
        w_prof = self.meta.profile(w_unit.card_id)
        cycle_cap = max(28, int(2.6 * obs.turn))
        is_cycle_slot = w_stats < cycle_cap or (w_prof is not None and w_prof.has_on_sell)

        results: List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []
        sell_act = seaglass.TavernAction.sell(w_slot)
        if not g.is_legal(sell_act):
            return []

        pre_sig = _plan_validation_key(obs)
        for h_idx, h_card in enumerate(obs.hand):
            if h_card.is_spell or h_card.unplayable:
                continue
            h_prof = self.meta.profile(h_card.card_id)
            h_stats = h_card.attack + h_card.health
            is_merged_gold = h_card.is_golden and not (
                h_prof is not None and h_prof.is_intrinsic_golden
            )
            can_replace_big = (
                h_stats >= 0.70 * w_stats
                or (is_merged_gold and w_stats < 55)
                or (
                    h_prof is not None
                    and h_prof.is_scaling_engine
                    and h_card.tavern_tier >= 5
                    and w_stats < 55
                    and not (w_prof is not None and w_prof.is_scaling_engine)
                )
            )
            if not (is_cycle_slot or can_replace_big):
                continue

            mid = g.copy()
            mid.reseed(self.rng.getrandbits(63) | 1)
            mid.step(sell_act)
            mid_obs = mid.observe()
            if mid_obs.discover_pending is not None or h_idx >= len(mid_obs.hand):
                continue
            ins_pos = min(w_slot, len(mid_obs.board))
            play_act = seaglass.TavernAction.play(h_idx, ins_pos)
            if not mid.is_legal(play_act):
                continue
            mid_sig = _plan_validation_key(mid_obs)
            step1: QueuedStep = (_action_key(sell_act), pre_sig, False)
            mid.step(play_act)
            after_obs = mid.observe()
            # Stop queue if playing this minion generated a random card in hand!
            gen_card = len(after_obs.hand) >= len(mid_obs.hand)
            step2: QueuedStep = (_action_key(play_act), mid_sig, gen_card)
            results.append((mid, after_obs, path + [step1, step2]))
        return results

    def _search_shop_epoch(
        self,
        root_game: seaglass.TavernGame,
        particles: OpponentParticleSet,
        ctx: Optional[seaglass.LobbyContext],
    ) -> Tuple[float, List[QueuedStep]]:
        """Deterministic DAG Beam Search within the currently visible shop (batched evaluation)."""
        root_obs = root_game.observe()
        root_fast = self.evaluator.evaluate_fast(root_obs, ctx=ctx, end_of_turn=False)
        transposition_best: Dict[Tuple, float] = {_canonical_state_key(root_obs): root_fast}

        beam: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = [
            (root_fast, root_game, root_obs, [])
        ]

        # Collect un-scored terminal candidates for batched scoring at the end of beam expansion!
        raw_terminals: List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, bool, List[QueuedStep]]] = []

        for _depth in range(self.max_epoch_depth):
            raw_children: List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []

            for _, g, obs, path in beam:
                obs_sig = _plan_validation_key(obs)
                end_step: QueuedStep = (("end_turn", None, None), obs_sig, False)
                raw_terminals.append((g, obs, True, path + [end_step]))

                if not obs.is_frozen and obs.shop and obs.gold < obs.minion_cost:
                    fz_act = seaglass.TavernAction.toggle_freeze()
                    if g.is_legal(fz_act):
                        g_fz = g.copy()
                        g_fz.step(fz_act)
                        fz_obs = g_fz.observe()
                        fz_step: QueuedStep = (("toggle_freeze", None, None), obs_sig, False)
                        fz_end: QueuedStep = (
                            ("end_turn", None, None),
                            _plan_validation_key(fz_obs),
                            False,
                        )
                        raw_terminals.append((g_fz, fz_obs, True, path + [fz_step, fz_end]))

                if obs.discover_pending is not None or obs.trinket_options:
                    raw_terminals.append((g, obs, False, path))
                    continue

                # 1. Standard pruned single-step transitions.
                legal = prune_tavern_actions(
                    obs, g.legal_actions(), self.meta, legacy_mode=self.legacy_mode
                )
                for act in legal:
                    k = act.kind
                    if k in ("end_turn", "toggle_freeze", "refresh", "dark_discovery"):
                        continue
                    if k == "hero_power" and self._is_stochastic_or_shop_hero_power(g, obs, act):
                        continue

                    child = g.copy()
                    child.reseed(self.rng.getrandbits(63) | 1)
                    ended = child.step(act)
                    c_obs = child.observe()

                    # Detect stochastic card/shop/gold-generating actions (`Enchanted Lasso`,
                    # `Recruit a Trainee`, random summons/transforms) so we NEVER blindly execute
                    # downstream `Sell` steps before observing the realized draw!
                    is_stoch_step = False
                    if (
                        not self.legacy_mode
                        and k in ("play", "activate", "sell")
                        and c_obs.discover_pending is None
                    ):
                        probe = g.copy()
                        probe.reseed(999983)
                        probe.step(act)
                        if _decision_skeleton(probe.observe()) != _decision_skeleton(c_obs):
                            is_stoch_step = True

                    q_step: QueuedStep = (_action_key(act), obs_sig, is_stoch_step)
                    if ended:
                        raw_terminals.append((child, c_obs, True, path + [q_step]))
                    elif is_stoch_step:
                        # Treat stochastic spell/Battlecry draw as a lookahead leaf boundary!
                        raw_terminals.append((child, c_obs, False, path + [q_step]))
                    else:
                        raw_children.append((child, c_obs, path + [q_step]))

                # 2. Compound full-board Cycle Macro transitions (`Sell(weak) + Play(hand)`).
                raw_children.extend(self._expand_cycle_macros(g, obs, path))

            if not raw_children:
                break

            # Score all unique child states in a SINGLE batched neural+prior call!
            dedup_children: List[Tuple[Tuple, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []
            seen_layer: set[Tuple] = set()
            for child, c_obs, c_path in raw_children:
                ckey = _canonical_state_key(c_obs)
                if ckey in seen_layer:
                    continue
                seen_layer.add(ckey)
                dedup_children.append((ckey, child, c_obs, c_path))

            child_scores = self.evaluator.evaluate_fast_batch(
                [item[2] for item in dedup_children], ctx=ctx, end_of_turn=False
            )

            next_beam: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []
            for (ckey, child, c_obs, c_path), c_fast in zip(dedup_children, child_scores):
                prev = transposition_best.get(ckey)
                if prev is not None and c_fast <= prev + 1e-6:
                    continue
                transposition_best[ckey] = c_fast
                next_beam.append((c_fast, child, c_obs, c_path))

            if not next_beam:
                break
            next_beam.sort(key=lambda x: x[0], reverse=True)
            beam = next_beam[: self.beam_width]

        # Batch-score terminal candidates (`end_of_turn=True` vs `False` for open Discover leaves).
        eot_indices = [i for i, item in enumerate(raw_terminals) if item[2]]
        open_indices = [i for i, item in enumerate(raw_terminals) if not item[2]]
        term_scores = [0.0] * len(raw_terminals)

        if eot_indices:
            vals = self.evaluator.evaluate_fast_batch(
                [raw_terminals[i][1] for i in eot_indices], ctx=ctx, end_of_turn=True
            )
            for idx, v in zip(eot_indices, vals):
                term_scores[idx] = v
        if open_indices:
            vals = self.evaluator.evaluate_fast_batch(
                [raw_terminals[i][1] for i in open_indices], ctx=ctx, end_of_turn=False
            )
            for idx, v in zip(open_indices, vals):
                term_scores[idx] = v

        ranked_terms: List[Tuple[float, seaglass.TavernGame, List[QueuedStep]]] = []
        seen_term_keys: set[Tuple] = set()
        for t_score, (t_game, t_obs, t_eot, t_path) in sorted(
            zip(term_scores, raw_terminals), key=lambda x: x[0], reverse=True
        ):
            t_sig = (_canonical_state_key(t_obs), t_eot)
            if t_sig in seen_term_keys:
                continue
            seen_term_keys.add(t_sig)
            ranked_terms.append((t_score, t_game, t_path))
            if len(ranked_terms) >= self.top_k_combat_evals:
                break

        best_score = -1e18
        default_end: QueuedStep = (
            ("end_turn", None, None),
            _plan_validation_key(root_obs),
            False,
        )
        best_path: List[QueuedStep] = [default_end]

        for _, cand_game, cand_path in ranked_terms:
            # If candidate stops at a Discover prompt or a stochastic card-draw boundary
            # (e.g. `Enchanted Lasso`), resolve expected continuation on copies so its real value is scored!
            eval_game = cand_game
            cand_obs = cand_game.observe()
            if cand_obs.discover_pending is not None:
                eval_game = cand_game.copy()
                eval_game.reseed(self.rng.getrandbits(63) | 1)
                d_legal = eval_game.legal_actions()
                if d_legal:
                    d_pick = self._evaluate_prompt_options(eval_game, d_legal, particles, ctx)
                    eval_game.step(d_pick)
            elif cand_path and cand_path[-1][2]:
                # Roll out greedy continuation on cand_game after stochastic card draw!
                eval_game = self._greedy_roll_in_copy(cand_game, ctx)

            full_score = self.evaluator.evaluate_turn_end(
                eval_game,
                particles,
                ctx=ctx,
                combats_per_particle=12,
            )
            if full_score > best_score:
                best_score = full_score
                best_path = cand_path

        return best_score, best_path

    def _greedy_roll_in_copy(
        self,
        start_game: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
    ) -> seaglass.TavernGame:
        """Fast 3-step greedy roll-in after a stochastic draw or shop refresh."""
        sim = start_game.copy()
        sim.reseed(self.rng.getrandbits(63) | 1)
        for _step in range(3):
            obs = sim.observe()
            if obs.discover_pending is not None:
                d_acts = sim.legal_actions()
                if d_acts:
                    sim.step(d_acts[0])
                continue
            stop_val = self.evaluator.evaluate_fast(obs, ctx=ctx, end_of_turn=True)
            cand_sims: List[seaglass.TavernGame] = []
            cand_obs: List[seaglass.TavernObservation] = []
            for act in prune_tavern_actions(
                obs, sim.legal_actions(), self.meta, legacy_mode=self.legacy_mode
            ):
                if act.kind in ("end_turn", "refresh", "toggle_freeze"):
                    continue
                if act.kind == "hero_power" and self._is_stochastic_or_shop_hero_power(sim, obs, act):
                    continue
                c = sim.copy()
                c.reseed(self.rng.getrandbits(63) | 1)
                c.step(act)
                cand_sims.append(c)
                cand_obs.append(c.observe())

            for c_game, c_o, _ in self._expand_cycle_macros(sim, obs, []):
                cand_sims.append(c_game)
                cand_obs.append(c_o)

            if not cand_sims:
                break
            scores = self.evaluator.evaluate_fast_batch(cand_obs, ctx=ctx, end_of_turn=False)
            best_idx = max(range(len(scores)), key=lambda i: scores[i])
            if scores[best_idx] > stop_val + 0.35:
                sim = cand_sims[best_idx]
            else:
                break
        return sim

    def _evaluate_stochastic_action(
        self,
        root_game: seaglass.TavernGame,
        stoch_act: seaglass.TavernAction,
        particles: OpponentParticleSet,
        ctx: Optional[seaglass.LobbyContext],
    ) -> float:
        """Estimate E[V(stoch_act)] across `refresh_samples` reseeded chance rollouts."""
        total = 0.0
        for _ in range(self.refresh_samples):
            sim = root_game.copy()
            sim.reseed(self.rng.getrandbits(63) | 1)
            sim.step(stoch_act)
            legal_after = sim.legal_actions()
            if legal_after and legal_after[0].kind == "choose_discover":
                pick = self._evaluate_prompt_options(sim, legal_after, particles, ctx)
                sim.step(pick)

            sim = self._greedy_roll_in_copy(sim, ctx)
            total += self.evaluator.evaluate_turn_end(
                sim, particles, ctx=ctx, combats_per_particle=8
            )
        return total / max(1, self.refresh_samples)

    def choose_action_for_tavern(
        self,
        belief_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        player: int = 0,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> seaglass.TavernAction:
        obs = belief_game.observe()
        self.last_note = None
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            return legal[0]

        if legal[0].kind == "choose_hero":
            return choose_best_hero_action(
                obs, legal, self.meta, base_game=belief_game, evaluator=self.evaluator
            )

        particles = self.belief.next_opponent_particles(obs.turn, player, ctx)

        if legal[0].kind in ("choose_discover", "choose_trinket"):
            self._queued_actions.pop(player, None)
            best_opt = self._evaluate_prompt_options(belief_game, legal, particles, ctx)
            self.last_note = {"value": float(self.evaluator.evaluate_fast(obs, ctx=ctx))}
            return best_opt

        # Validate & execute queued plan ONLY if full deterministic state signature matches!
        queued = self._queued_actions.get(player)
        if queued:
            next_key, exp_sig, stop_after = queued.pop(0)
            if _plan_validation_key(obs) == exp_sig:
                if next_key[0] == "end_turn":
                    self._queued_actions.pop(player, None)
                    return self._finalize_positioning_or_end(
                        belief_game, legal, player, obs.turn, particles, ctx
                    )
                matched = self._find_matching_legal(legal, next_key)
                if matched is not None:
                    if stop_after or not queued:
                        self._queued_actions.pop(player, None)
                    self.last_note = {"queued_plan": float(len(queued) + 1)}
                    return matched
            self._queued_actions.pop(player, None)

        epoch_score, epoch_path = self._search_shop_epoch(belief_game, particles, ctx)

        best_stoch_act: Optional[seaglass.TavernAction] = None
        best_stoch_score = epoch_score
        roll_ev_logged: Optional[float] = None

        dd_act = seaglass.TavernAction.dark_discovery()
        if belief_game.is_legal(dd_act):
            dd_score = self._evaluate_stochastic_action(
                belief_game, dd_act, particles, ctx
            )
            if dd_score > best_stoch_score + 0.25:
                best_stoch_score = dd_score
                best_stoch_act = dd_act

        # Evaluate shop-mutating / stochastic Hero Powers (`Bob's Burgles`, `Temporal Tavern`, etc.)
        # across `refresh_samples` RNG rollouts instead of single-seed hallucination!
        if not self.legacy_mode:
            for act in legal:
                if act.kind == "hero_power" and self._is_stochastic_or_shop_hero_power(
                    belief_game, obs, act
                ):
                    hp_score = self._evaluate_stochastic_action(
                        belief_game, act, particles, ctx
                    )
                    if hp_score > best_stoch_score + 0.35:
                        best_stoch_score = hp_score
                        best_stoch_act = act

        ref_act = seaglass.TavernAction.refresh()
        can_act_after_roll = (obs.free_refreshes > 0 and obs.gold >= 1) or (
            obs.gold >= obs.refresh_cost + 1
        )
        if belief_game.is_legal(ref_act) and can_act_after_roll:
            ref_score = self._evaluate_stochastic_action(
                belief_game, ref_act, particles, ctx
            )
            roll_ev_logged = ref_score
            if ref_score > best_stoch_score + 0.35:
                best_stoch_score = ref_score
                best_stoch_act = ref_act

        note_dict: Dict[str, float] = {
            "value": round(float(best_stoch_score), 3),
            "epoch_ev": round(float(epoch_score), 3),
        }
        if roll_ev_logged is not None:
            note_dict["roll_ev"] = round(float(roll_ev_logged), 3)
        self.last_note = note_dict

        if best_stoch_act is not None:
            self._queued_actions.pop(player, None)
            matched = self._find_matching_legal(legal, _action_key(best_stoch_act))
            if matched is not None:
                return matched

        if not epoch_path or epoch_path[0][0][0] == "end_turn":
            return self._finalize_positioning_or_end(
                belief_game, legal, player, obs.turn, particles, ctx
            )

        first_key, _, first_stop = epoch_path[0]
        if len(epoch_path) > 1 and not first_stop:
            # Truncate queued plan at the first stochastic step boundary!
            trimmed: List[QueuedStep] = []
            for st in epoch_path[1:]:
                trimmed.append(st)
                if st[2]:
                    break
            self._queued_actions[player] = trimmed
        else:
            self._queued_actions.pop(player, None)

        matched = self._find_matching_legal(legal, first_key)
        if matched is not None:
            return matched
        self._queued_actions.pop(player, None)
        return self._finalize_positioning_or_end(
            belief_game, legal, player, obs.turn, particles, ctx
        )

    def _finalize_positioning_or_end(
        self,
        belief_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        player: int,
        turn: int,
        particles: OpponentParticleSet,
        ctx: Optional[seaglass.LobbyContext],
    ) -> seaglass.TavernAction:
        """Run PositioningSolver once right before EndTurn, queueing exact Reposition moves."""
        obs = belief_game.observe()
        if self._positioned_turn.get(player) != turn and len(obs.board) >= 2:
            self._positioned_turn[player] = turn
            perm = self.positioner.best_permutation(belief_game, particles, ctx=ctx)
            repo_acts = self.positioner.permutation_to_actions(perm)
            if repo_acts:
                sim_pos = belief_game.copy()
                steps: List[QueuedStep] = []
                for a in repo_acts:
                    steps.append((_action_key(a), _plan_validation_key(sim_pos.observe()), False))
                    if sim_pos.is_legal(a):
                        sim_pos.step(a)
                steps.append((("end_turn", None, None), _plan_validation_key(sim_pos.observe()), False))
                first = self._find_matching_legal(legal, steps[0][0])
                if first is not None:
                    self._queued_actions[player] = steps[1:]
                    self.last_note = {"pos_swaps": float(len(repo_acts))}
                    return first

        end_act = self._find_matching_legal(legal, ("end_turn", None, None))
        return end_act if end_act is not None else legal[0]

    def act_in_lobby(
        self, lobby: seaglass.LobbyGame, player: int
    ) -> seaglass.TavernAction:
        legal = lobby.legal_actions(player)
        if not legal:
            return seaglass.TavernAction.end_turn()
        if lobby.phase == "hero":
            obs = lobby.observe(player)
            depletion = self.belief.sample_pool_depletion_units(lobby.turn, None)
            bg = lobby.belief_tavern(player, seed=self.rng.getrandbits(63) | 1, extra_units=depletion)
            return choose_best_hero_action(
                obs, legal, self.meta, base_game=bg, evaluator=self.evaluator
            )

        ctx = lobby.context(player)
        depletion = self.belief.sample_pool_depletion_units(lobby.turn, ctx)
        belief_game = lobby.belief_tavern(
            player, seed=self.rng.getrandbits(63) | 1, extra_units=depletion
        )
        return self.choose_action_for_tavern(
            belief_game, legal, player=player, ctx=ctx
        )


@dataclass
class _DecisionNode:
    game: seaglass.TavernGame
    actions: List[seaglass.TavernAction]
    priors: List[float]
    visits: int = 0
    value_sum: float = 0.0
    det_children: Dict[int, "_DecisionNode"] = field(default_factory=dict)
    chance_children: Dict[int, "_ChanceNode"] = field(default_factory=dict)
    is_terminal: bool = False


@dataclass
class _ChanceNode:
    parent_game: seaglass.TavernGame
    action: seaglass.TavernAction
    visits: int = 0
    value_sum: float = 0.0
    outcomes: List[_DecisionNode] = field(default_factory=list)


class ExpectimaxMctsBot:
    """Expectimax Monte Carlo Tree Search with PUCT & Progressive Widening at Chance Nodes."""

    def __init__(
        self,
        catalog_name: str = "full",
        num_simulations: int = 64,
        c_puct: float = 1.6,
        pw_alpha: float = 0.5,
        max_tree_depth: int = 10,
        num_particles: int = 3,
        seed: int = 99,
    ) -> None:
        self.meta = get_meta_db(catalog_name)
        self.belief = LobbyBeliefFilter(
            catalog_name=catalog_name, num_particles=num_particles, seed=seed
        )
        self.evaluator = LeafEvaluator(catalog_name=catalog_name)
        self.positioner = PositioningSolver(
            catalog_name=catalog_name, max_candidates=48, seed=seed + 13, evaluator=self.evaluator
        )
        self.num_simulations = num_simulations
        self.c_puct = c_puct
        self.pw_alpha = pw_alpha
        self.max_tree_depth = max_tree_depth
        self.rng = random.Random(seed)
        self._queued_repos: Dict[int, List[ActionKey]] = {}
        self._positioned_turn: Dict[int, int] = {}

    @staticmethod
    def _is_stochastic_action(
        obs: seaglass.TavernObservation, act: seaglass.TavernAction
    ) -> bool:
        if act.kind in ("refresh", "dark_discovery"):
            return True
        if act.kind == "play":
            h_idx, _ = act.args
            if h_idx is not None and h_idx < len(obs.hand) and obs.hand[h_idx].is_golden:
                return True
        return False

    def _make_decision_node(
        self,
        game: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        is_terminal: bool = False,
    ) -> _DecisionNode:
        _ = ctx
        if is_terminal:
            return _DecisionNode(game=game, actions=[], priors=[], is_terminal=True)
        obs = game.observe()
        acts = prune_tavern_actions(obs, game.legal_actions(), self.meta)
        if not acts:
            return _DecisionNode(game=game, actions=[], priors=[], is_terminal=True)

        logits: List[float] = []
        for a in acts:
            k = a.kind
            if k == "play":
                logits.append(2.2)
            elif k == "buy":
                logits.append(1.5)
            elif k == "upgrade_tavern":
                logits.append(1.7 if obs.upgrade_cost <= max(5, obs.gold - 1) else 0.8)
            elif k in ("choose_discover", "choose_trinket"):
                logits.append(2.0)
            elif k in ("hero_power", "activate"):
                logits.append(1.2)
            elif k == "refresh":
                logits.append(0.8 if obs.gold >= 4 or obs.free_refreshes > 0 else -0.8)
            elif k == "end_turn":
                logits.append(1.4 if obs.gold < 2 and not obs.hand else -0.5)
            elif k == "sell":
                logits.append(0.5 if len(obs.board) >= 7 else -1.0)
            else:
                logits.append(0.0)
        max_l = max(logits)
        exps = [math.exp(l - max_l) for l in logits]
        s = sum(exps)
        priors = [e / s for e in exps]
        return _DecisionNode(game=game, actions=acts, priors=priors, is_terminal=False)

    def _select_puct(self, node: _DecisionNode, root_scale: float) -> int:
        sqrt_n = math.sqrt(max(1, node.visits))
        best_idx = 0
        best_ucb = -1e18
        parent_q = (node.value_sum / node.visits) / root_scale if node.visits > 0 else 0.0

        for i in range(len(node.actions)):
            if i in node.det_children:
                ch = node.det_children[i]
                n_sa = ch.visits
                q_sa = (ch.value_sum / n_sa) / root_scale if n_sa > 0 else parent_q
            elif i in node.chance_children:
                ch_c = node.chance_children[i]
                n_sa = ch_c.visits
                q_sa = (ch_c.value_sum / n_sa) / root_scale if n_sa > 0 else parent_q
            else:
                n_sa = 0
                q_sa = parent_q

            u = q_sa + self.c_puct * node.priors[i] * sqrt_n / (1.0 + n_sa)
            if u > best_ucb:
                best_ucb = u
                best_idx = i
        return best_idx

    def _leaf_value(
        self,
        node: _DecisionNode,
        particles: OpponentParticleSet,
        ctx: Optional[seaglass.LobbyContext],
    ) -> float:
        if node.is_terminal:
            return self.evaluator.evaluate_turn_end(
                node.game, particles, ctx=ctx, combats_per_particle=8
            )
        sim = node.game.copy()
        sim.reseed(self.rng.getrandbits(63) | 1)
        for _ in range(3):
            obs = sim.observe()
            if obs.gold == 0 and not any(not c.unplayable for c in obs.hand):
                break
            acts = prune_tavern_actions(obs, sim.legal_actions(), self.meta)
            play_acts = [a for a in acts if a.kind in ("choose_discover", "play")]
            if play_acts:
                sim.step(play_acts[0])
                continue
            buy_acts = [a for a in acts if a.kind == "buy"]
            if buy_acts and len(obs.board) < 7:
                sim.step(buy_acts[0])
                continue
            break
        return self.evaluator.evaluate_turn_end(
            sim, particles, ctx=ctx, combats_per_particle=6
        )

    def act_in_lobby(
        self, lobby: seaglass.LobbyGame, player: int
    ) -> seaglass.TavernAction:
        legal = lobby.legal_actions(player)
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            return legal[0]
        if lobby.phase == "hero":
            return choose_best_hero_action(lobby.observe(player), legal, self.meta)

        queued = self._queued_repos.get(player)
        if queued:
            next_k = queued.pop(0)
            for a in legal:
                if _action_key(a) == next_k:
                    return a
            self._queued_repos.pop(player, None)

        ctx = lobby.context(player)
        depletion = self.belief.sample_pool_depletion_units(lobby.turn, ctx)
        root_game = lobby.belief_tavern(
            player, seed=self.rng.getrandbits(63) | 1, extra_units=depletion
        )
        obs = root_game.observe()
        particles = self.belief.next_opponent_particles(obs.turn, player, ctx)

        root = self._make_decision_node(root_game, ctx)
        if not root.actions:
            return legal[0]
        if len(root.actions) == 1:
            chosen_key = _action_key(root.actions[0])
        else:
            root_scale = 14.0
            for _ in range(self.num_simulations):
                path_dec: List[_DecisionNode] = [root]
                path_ch: List[_ChanceNode] = []
                cur = root
                depth = 0

                while not cur.is_terminal and depth < self.max_tree_depth:
                    a_idx = self._select_puct(cur, root_scale)
                    act = cur.actions[a_idx]
                    cur_obs = cur.game.observe()

                    if act.kind == "end_turn":
                        if a_idx not in cur.det_children:
                            g_end = cur.game.copy()
                            cur.det_children[a_idx] = self._make_decision_node(
                                g_end, ctx, is_terminal=True
                            )
                        cur = cur.det_children[a_idx]
                        path_dec.append(cur)
                        break

                    if self._is_stochastic_action(cur_obs, act):
                        ch_node = cur.chance_children.get(a_idx)
                        if ch_node is None:
                            ch_node = _ChanceNode(parent_game=cur.game, action=act)
                            cur.chance_children[a_idx] = ch_node
                        path_ch.append(ch_node)

                        max_outcomes = max(1, int((ch_node.visits + 1) ** self.pw_alpha))
                        if len(ch_node.outcomes) < max_outcomes:
                            g_next = ch_node.parent_game.copy()
                            g_next.reseed(self.rng.getrandbits(63) | 1)
                            ended = g_next.step(act)
                            d_next = self._make_decision_node(g_next, ctx, is_terminal=ended)
                            ch_node.outcomes.append(d_next)
                            cur = d_next
                            path_dec.append(cur)
                            break
                        else:
                            cur = self.rng.choice(ch_node.outcomes)
                            path_dec.append(cur)
                    else:
                        if a_idx not in cur.det_children:
                            g_next = cur.game.copy()
                            g_next.reseed(self.rng.getrandbits(63) | 1)
                            ended = g_next.step(act)
                            cur.det_children[a_idx] = self._make_decision_node(
                                g_next, ctx, is_terminal=ended
                            )
                            cur = cur.det_children[a_idx]
                            path_dec.append(cur)
                            break
                        cur = cur.det_children[a_idx]
                        path_dec.append(cur)

                    depth += 1

                val = self._leaf_value(cur, particles, ctx)
                for dn in path_dec:
                    dn.visits += 1
                    dn.value_sum += val
                for cn in path_ch:
                    cn.visits += 1
                    cn.value_sum += val

            best_i = max(
                range(len(root.actions)),
                key=lambda i: (
                    root.det_children[i].visits
                    if i in root.det_children
                    else (root.chance_children[i].visits if i in root.chance_children else 0),
                    root.det_children[i].value_sum / max(1, root.det_children[i].visits)
                    if i in root.det_children
                    else (
                        root.chance_children[i].value_sum / max(1, root.chance_children[i].visits)
                        if i in root.chance_children
                        else -1e18
                    ),
                ),
            )
            chosen_key = _action_key(root.actions[best_i])

        if chosen_key[0] == "end_turn":
            if self._positioned_turn.get(player) != obs.turn and len(obs.board) >= 2:
                self._positioned_turn[player] = obs.turn
                perm = self.positioner.best_permutation(root_game, particles, ctx=ctx)
                repos = self.positioner.permutation_to_actions(perm)
                if repos:
                    keys = [_action_key(r) for r in repos] + [("end_turn", None, None)]
                    for a in legal:
                        if _action_key(a) == keys[0]:
                            self._queued_repos[player] = keys[1:]
                            return a

        for a in legal:
            if _action_key(a) == chosen_key:
                return a
        return legal[0]
