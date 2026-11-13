"""ApexBot: Continuation-Aware Multi-Epoch Beam + Monte Carlo Physics Rollout Bot (Solo & Duos)."""

from __future__ import annotations

import os
import random
from typing import Any, Dict, List, Optional, Sequence, Tuple

import seaglass

from .apex_belief import ApexBeliefTracker, ApexOpponentParticles
from .apex_evaluator import ApexEvaluator
from .apex_knowledge import TRINKET_EQUITY_PRIORS, ApexKnowledge, get_apex_knowledge
from .apex_positioning import ApexPositioner

ActionKey = Tuple[str, Optional[int], Optional[int]]
# Queued step: (action_key, expected_plan_validation_key, stop_queue_after_step)
QueuedStep = Tuple[ActionKey, Tuple, bool]


def _action_key(a: seaglass.TavernAction) -> ActionKey:
    arg0, arg1 = a.args
    return (a.kind, arg0, arg1)


def _plan_validation_key(obs: seaglass.TavernObservation) -> Tuple:
    """
    Order-invariant board + exact hand/shop/gold state invariant for 0.02ms queued-step execution.
    Any unexpected random card generation, shop mutation, or Discover prompt invalidates the queue!
    """
    board_multiset = tuple(
        sorted((u.card_id, u.attack, u.health, u.is_golden, u.divine_shield, u.venomous) for u in obs.board)
    )
    hand_exact = tuple(
        (c.card_id, c.attack, c.health, c.is_golden, c.is_spell, c.unplayable) for c in obs.hand
    )
    shop_exact = tuple(
        (s.card_id, s.attack, s.health, s.is_spell, s.spell_cost) for s in obs.shop
    )
    return (
        obs.gold,
        obs.tavern_tier,
        obs.upgrade_cost,
        obs.free_refreshes,
        obs.hero_power_uses_left,
        getattr(obs, "second_hero_power_uses_left", 0),
        getattr(obs, "dark_discoveries_left", 0),
        obs.discover_pending,
        bool(obs.trinket_options),
        tuple(obs.effects),
        tuple(obs.trinket_counters),
        board_multiset,
        hand_exact,
        shop_exact,
    )


def _decision_skeleton(obs: seaglass.TavernObservation) -> Tuple:
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
        (c.card_id, c.attack, c.health, c.is_golden, c.is_spell, c.unplayable)
        for c in obs.hand
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
        getattr(obs, "second_hero_power_uses_left", 0),
        getattr(obs, "dark_discoveries_left", 0),
        board_sig,
        hand_sig,
        shop_sig,
        tuple(obs.player_counters),
        tuple(obs.effects),
        tuple(obs.trinket_counters),
    )


def expendable_slots(
    obs: seaglass.TavernObservation,
    evaluator: ApexEvaluator,
    max_slots: int = 2,
    particles: Optional[ApexOpponentParticles] = None,
) -> List[int]:
    """Return lowest keep-priority board indices (protecting compounding engines & scam tech, favoring on-sell)."""
    n = len(obs.board)
    if n == 0:
        return []
    tribe_counts: Dict[str, int] = {}
    scam_count = 0
    leeroy_id = evaluator.kb.leeroy_id
    morgl_id = evaluator.kb.morgl_id
    for u in obs.board:
        if u.venomous or u.card_id in (leeroy_id, morgl_id):
            scam_count += 1
        for tr in u.tribes:
            tribe_counts[tr] = tribe_counts.get(tr, 0) + 1
    ranked = [
        (
            evaluator.unit_keep_priority(
                u,
                obs.turn,
                tribe_counts,
                scam_count=scam_count,
                particles=particles,
                board_units=obs.board,
                obs_trinkets=obs.trinkets,
            ),
            idx,
        )
        for idx, u in enumerate(obs.board)
    ]
    ranked.sort(key=lambda x: x[0])
    return [idx for _, idx in ranked[: min(n, max_slots)]]


def prune_apex_actions(
    obs: seaglass.TavernObservation,
    legal: Sequence[seaglass.TavernAction],
    evaluator: ApexEvaluator,
    kb: ApexKnowledge,
    particles: Optional[ApexOpponentParticles] = None,
) -> List[seaglass.TavernAction]:
    """
    Action space filter for ApexBot:
      - Preserves ALL target positions for `battlecry_target` minions & taught `Magicfin Apprentice`s,
      - Cures full-board calcification & 10-card hand paralysis while protecting compounding engines & scam tech,
      - Treats `Greedy Conniver` as `on_sell` ONLY when Golden,
      - Includes `activate`, `hero_power`, `hero_power_2`, and best `play_on_shop` targets.
    """
    meta = kb.meta
    out: List[seaglass.TavernAction] = []
    seen_play_minion_hand: Dict[int, bool] = {}

    tribe_counts: Dict[str, int] = {}
    for u in obs.board:
        for tr in u.tribes:
            tribe_counts[tr] = tribe_counts.get(tr, 0) + 1

    owns_gauntlet = any(tid in kb.weighted_gauntlet_ids for tid in obs.trinkets)
    eff_colossus = (
        max(particles.colossus_count, particles.lobby_colossus_count)
        if (particles is not None and not owns_gauntlet)
        else 0
    )
    drakkari_id = kb.name_to_id.get("Drakkari Enchanter", -1)

    weak_slots = expendable_slots(obs, evaluator, max_slots=2, particles=particles)
    sell_allowed = set(weak_slots)
    weakest_idx = weak_slots[0] if weak_slots else -1
    weakest_unit = obs.board[weakest_idx] if weakest_idx >= 0 else None
    weakest_stats = (
        (weakest_unit.attack + weakest_unit.health) if weakest_unit is not None else 0
    )
    weakest_eng = (
        evaluator.effective_engine_bonus(
            weakest_unit.card_id,
            obs.turn,
            tribe_counts,
            on_board=True,
            board_units=obs.board,
            is_golden=weakest_unit.is_golden,
            obs_trinkets=obs.trinkets,
        )
        if weakest_unit is not None
        else 0.0
    )

    cycle_stat_cap = max(32, int(3.4 * obs.turn))
    weakest_is_on_sell = bool(
        weakest_unit is not None
        and (
            weakest_unit.card_id in kb.true_on_sell_plain_ids
            or (weakest_unit.card_id in kb.golden_only_sell_ids and weakest_unit.is_golden)
        )
    )
    weakest_is_scam = bool(
        weakest_unit is not None
        and (weakest_unit.venomous or weakest_unit.card_id in (kb.leeroy_id, kb.morgl_id))
    )
    has_cycle_slot = weakest_idx >= 0 and (
        (weakest_stats < cycle_stat_cap and weakest_eng < 10.0 and not weakest_is_scam)
        or weakest_is_on_sell
    )

    has_mech_target = tribe_counts.get("mech", 0) > 0 or tribe_counts.get("all", 0) > 0
    hand_synergy = evaluator.has_hand_buff_synergy(obs)
    elem_surprise_id = kb.name_to_id.get("Elemental of Surprise", -1)
    hand_clogged = (
        len(obs.hand) >= 9
        and not hand_synergy
        and not any(c.is_spell and not c.unplayable for c in obs.hand)
    )

    owned_copies: Dict[int, int] = {}
    non_golden_elementals = 0
    for u in list(obs.board) + list(obs.hand):
        if not u.is_golden and not u.is_spell:
            owned_copies[u.card_id] = owned_copies.get(u.card_id, 0) + 1
            if "elemental" in u.tribes or "all" in u.tribes:
                non_golden_elementals += 1

    top_shop_targets: set[int] = set()
    if obs.shop:
        s_ranked = sorted(
            range(len(obs.shop)),
            key=lambda idx: (
                not obs.shop[idx].is_spell,
                obs.shop[idx].attack + obs.shop[idx].health,
                obs.shop[idx].tavern_tier,
            ),
            reverse=True,
        )
        top_shop_targets = set(s_ranked[:2])

    has_soul_rewinder = any(u.card_id == kb.soul_rewinder_id for u in obs.board)
    ehp_now = evaluator.effective_hp(obs)

    # Hand Symmetry Deduplication: collapse 100% identical hand cards (e.g. 8 Blood Gems -> 1)!
    shop_spells_in_hand = sum(
        1 for c in obs.hand if c.is_spell and c.card_id in meta.templates_by_id
    )
    allow_shop_spell_dedup = (
        kb.willbreaker_sticker_id not in obs.trinkets and shop_spells_in_hand < 3
    )
    seen_hand_sym: Dict[Tuple, int] = {}
    canonical_hand_idx: set[int] = set()
    for h_i, hc in enumerate(obs.hand):
        is_shop_spell = hc.is_spell and hc.card_id in meta.templates_by_id
        if is_shop_spell and not allow_shop_spell_dedup:
            canonical_hand_idx.add(h_i)
            continue
        sym_k = (
            hc.card_id,
            hc.is_golden,
            hc.attack,
            hc.health,
            hc.spell_cost,
            hc.counter,
            hc.unplayable,
            tuple(hc.dark_gifts),
            (h_i == 0) if (hand_synergy and not hc.is_spell) else False,
        )
        if sym_k not in seen_hand_sym:
            seen_hand_sym[sym_k] = h_i
            canonical_hand_idx.add(h_i)

    for act in legal:
        k = act.kind
        if k == "reposition":
            continue

        if k == "sell":
            b_pos, _ = act.args
            if b_pos is None or b_pos >= len(obs.board):
                continue
            u_sell = obs.board[b_pos]
            u_eng = evaluator.effective_engine_bonus(
                u_sell.card_id,
                obs.turn,
                tribe_counts,
                on_board=True,
                board_units=obs.board,
                is_golden=u_sell.is_golden,
                obs_trinkets=obs.trinkets,
            )
            is_true_sell = (
                u_sell.card_id in kb.true_on_sell_plain_ids
                or (u_sell.card_id in kb.golden_only_sell_ids and u_sell.is_golden)
            )
            u_stats = u_sell.attack + u_sell.health
            u_is_scam = u_sell.venomous or u_sell.card_id in (kb.leeroy_id, kb.morgl_id)

            if len(obs.board) < 7:
                # Allow selling true `on_sell` economy/discover units (`Patient Scout`, `Sellemental`,
                # Golden `Greedy Conniver`) at any turn (end_of_turn discipline guarantees slot refill!).
                if is_true_sell and u_eng < 10.0 and not u_sell.venomous:
                    out.append(act)
                elif (
                    3 <= obs.turn <= 7
                    and b_pos == weakest_idx
                    and not u_sell.is_golden
                    and u_eng <= 0.0
                    and u_stats <= 10
                    and (
                        obs.gold + 1 == obs.minion_cost
                        or obs.gold + 1 == obs.upgrade_cost
                    )
                ):
                    out.append(act)
                continue

            hand_has_major_upgrade = any(
                not c.is_spell
                and not c.unplayable
                and (
                    c.is_golden
                    or evaluator.effective_engine_bonus(c.card_id, obs.turn, tribe_counts, on_board=False) > max(9.0, weakest_eng + 3.0)
                    or ((c.attack + c.health) > u_stats + 8 and u_eng < 10.0 and not u_is_scam)
                    or (owns_gauntlet and c.card_id == drakkari_id and u_sell.card_id != drakkari_id)
                    or (
                        not u_is_scam
                        and (c.venomous or c.card_id in (kb.leeroy_id, kb.morgl_id, kb.sindorei_id))
                        and (u_stats < 420 or eff_colossus >= 3)
                    )
                )
                for c in obs.hand
            )
            emergency_unclog = (
                hand_clogged
                and obs.gold >= 5
                and b_pos == weakest_idx
                and not u_sell.divine_shield
                and u_sell.card_id != drakkari_id
            )
            if b_pos in sell_allowed and (
                is_true_sell
                or (u_stats < cycle_stat_cap and u_eng < 10.0 and not u_is_scam)
                or (b_pos == weakest_idx and hand_has_major_upgrade and (u_eng < 12.0 or (owns_gauntlet and u_sell.card_id != drakkari_id)))
                or emergency_unclog
            ):
                out.append(act)
            continue

        if k == "buy":
            s_idx, _ = act.args
            if s_idx is not None and s_idx < len(obs.shop):
                s_card = obs.shop[s_idx]
                if s_card.costs_health and not has_soul_rewinder:
                    h_price = s_card.spell_cost if s_card.is_spell else obs.minion_cost
                    if ehp_now <= 18.0 or (ehp_now - h_price) <= 14.0:
                        continue
                if len(obs.board) >= 7 and not s_card.is_spell:
                    is_triple = (
                        not s_card.is_golden
                        and (
                            owned_copies.get(s_card.card_id, 0) >= 2
                            or (s_card.card_id == elem_surprise_id and non_golden_elementals >= 2)
                        )
                    )
                    is_mag = s_card.magnetic and has_mech_target
                    s_eng = evaluator.effective_engine_bonus(
                        s_card.card_id,
                        obs.turn,
                        tribe_counts,
                        on_board=False,
                        board_units=obs.board,
                        obs_trinkets=obs.trinkets,
                    )
                    s_stats = s_card.attack + s_card.health
                    is_gauntlet_drakkari = owns_gauntlet and s_card.card_id == drakkari_id
                    is_engine_pivot = (
                        is_gauntlet_drakkari
                        or (s_eng >= 10.0 and s_eng > weakest_eng + 2.5 and weakest_stats < 95)
                    )
                    is_tech = s_card.venomous or s_card.card_id in (
                        kb.leeroy_id,
                        kb.morgl_id,
                        kb.tyrael_id,
                        kb.sindorei_id,
                        kb.bile_spitter_id,
                    )
                    is_hand_murloc = hand_synergy and ("murloc" in s_card.tribes or "all" in s_card.tribes)
                    # Hand-Space Guard: when hand already has >= 7 cards on a full 7-minion board
                    # without handbuff synergy, do NOT clog hand with non-playable filler minions!
                    if len(obs.hand) >= 7 and not hand_synergy:
                        if not (
                            is_triple
                            or is_mag
                            or has_cycle_slot
                            or is_engine_pivot
                            or (is_tech and (eff_colossus >= 2 or weakest_stats < 180))
                        ):
                            continue
                    else:
                        is_stat_up = s_stats >= 0.65 * weakest_stats
                        if not (
                            is_triple
                            or is_mag
                            or has_cycle_slot
                            or is_engine_pivot
                            or is_tech
                            or is_hand_murloc
                            or is_stat_up
                        ):
                            continue

        if k in ("hero_power", "hero_power_2"):
            if obs.hero_id == kb.lich_bazhial_id and not has_soul_rewinder and ehp_now <= 20.0:
                continue
            if obs.hero_id in (kb.mutanus_id, kb.jailer_id):
                t_pos, _ = act.args
                if t_pos is not None and t_pos < len(obs.board):
                    tu = obs.board[t_pos]
                    t_eng = evaluator.effective_engine_bonus(
                        tu.card_id,
                        obs.turn,
                        tribe_counts,
                        on_board=True,
                        board_units=obs.board,
                        is_golden=tu.is_golden,
                        obs_trinkets=obs.trinkets,
                    )
                    if (
                        tu.is_golden
                        or tu.venomous
                        or tu.card_id in (kb.leeroy_id, kb.morgl_id)
                        or t_eng >= 9.0
                        or (obs.hero_id == kb.jailer_id and (tu.attack + tu.health) >= 24)
                    ):
                        continue

        if k == "activate":
            src_pos, dst_pos = act.args
            if src_pos is not None and src_pos < len(obs.board):
                src_u = obs.board[src_pos]
                if src_u.card_id == kb.deft_deserter_id:
                    has_shop_minion = any(not sc.is_spell for sc in obs.shop)
                    if not has_shop_minion or (obs.gold < 1 + obs.minion_cost and not obs.is_frozen):
                        continue
                if src_u.card_id == kb.tyrael_id and dst_pos is not None and dst_pos < len(obs.board):
                    dst_u = obs.board[dst_pos]
                    cap = 195 if src_u.is_golden else 95
                    if (dst_u.attack + dst_u.health) >= cap:
                        continue

        if k == "play":
            h_idx, b_pos = act.args
            if h_idx is not None and h_idx < len(obs.hand):
                if h_idx not in canonical_hand_idx:
                    continue
                card = obs.hand[h_idx]
                if card.is_spell:
                    if b_pos is not None and b_pos < len(obs.board):
                        tu = obs.board[b_pos]
                        t_stats = tu.attack + tu.health
                        if card.card_id == kb.robust_evolution_id:
                            t_eng = evaluator.effective_engine_bonus(
                                tu.card_id,
                                obs.turn,
                                tribe_counts,
                                on_board=True,
                                board_units=obs.board,
                                is_golden=tu.is_golden,
                            )
                            if (
                                tu.is_golden
                                or tu.venomous
                                or tu.card_id in (kb.leeroy_id, kb.morgl_id)
                                or tu.tavern_tier >= 6
                                or t_eng >= 8.0
                                or (tu.divine_shield and t_stats >= 30)
                                or t_stats >= 45
                            ):
                                continue
                        elif card.card_id in (847, 848):
                            t_eng = evaluator.effective_engine_bonus(
                                tu.card_id,
                                obs.turn,
                                tribe_counts,
                                on_board=True,
                                board_units=obs.board,
                                is_golden=tu.is_golden,
                            )
                            if (
                                tu.is_golden
                                or tu.venomous
                                or tu.card_id in (kb.leeroy_id, kb.morgl_id)
                                or t_eng >= 9.0
                                or (card.card_id == 848 and tu.divine_shield and t_stats >= 30)
                            ):
                                continue
                        elif card.card_id == 863 and t_stats >= 40:
                            continue
                else:
                    prof = meta.profile(card.card_id)
                    # Targeted Battlecries (`Captain Sanders`, `Elite Navigator`, `Maw Caster`,
                    # `Lovesick Balladist`, `Magicfin Apprentice`, etc.) choose target via `b_pos`!
                    is_targeted_bc = (
                        card.card_id in kb.targeted_battlecry_ids
                        or card.card_id not in meta.templates_by_id  # token minions like Magicfin Apprentice!
                    )
                    is_mag_target = (
                        card.magnetic
                        and b_pos is not None
                        and b_pos < len(obs.board)
                    )
                    has_bc = prof.has_battlecry if prof else False
                    if is_targeted_bc or is_mag_target:
                        out.append(act)
                        seen_play_minion_hand[h_idx] = True
                        continue
                    if has_bc and b_pos in (0, len(obs.board)):
                        out.append(act)
                        seen_play_minion_hand[h_idx] = True
                        continue
                    if not seen_play_minion_hand.get(h_idx, False) and b_pos == len(obs.board):
                        out.append(act)
                        seen_play_minion_hand[h_idx] = True
                        continue
                    continue

        if k == "play_on_shop":
            h_idx_ps, s_idx = act.args
            if h_idx_ps is not None and h_idx_ps not in canonical_hand_idx:
                continue
            if s_idx is not None and s_idx not in top_shop_targets:
                continue

        out.append(act)

    if not out and legal:
        return list(legal)
    return out


class ApexBot:
    """
    Champion Imperfect-Information Battlegrounds Bot (Solo & Duos):
      1. 0.02ms Queued Deterministic Step Fast-Path (skips belief_tavern rebuild on deterministic queues),
      2. Continuation-Aware Multi-Epoch DAG Beam Search (`Buy(Good) -> Refresh` prefix rollouts cure
         the spend-all-gold-on-garbage trap),
      3. 2-Turn Exact Rust Physics Rollouts on Turn 6 & Turn 9 `ChooseTrinket` offers,
      4. CVaR Zero-Death Survival Shield + Anti-Scam Tech + Dual-Head `ApexValueNet` (`.npz`),
      5. Engine-Exact `ApexPositioner` + Native Duos `Pass` Coordination.
    """

    def __init__(
        self,
        catalog_name: str = "full",
        beam_width: int = 20,
        max_epoch_depth: int = 9,
        top_k_combat_evals: int = 6,
        refresh_samples: int = 5,
        num_particles: int = 4,
        seed: int = 2027,
        evaluator: Optional[ApexEvaluator] = None,
        weights_path: Optional[os.PathLike | str] = None,
    ) -> None:
        self.catalog_name = catalog_name
        self.kb: ApexKnowledge = get_apex_knowledge(catalog_name)
        self.meta = self.kb.meta
        self.belief = ApexBeliefTracker(
            catalog_name=catalog_name, num_particles=num_particles, seed=seed
        )
        w_str = str(weights_path) if weights_path is not None else None
        self.evaluator = (
            evaluator
            if evaluator is not None
            else ApexEvaluator(catalog_name=catalog_name, weights_path=w_str)
        )
        self.positioner = ApexPositioner(
            catalog_name=catalog_name,
            max_candidates=48,
            evaluator=self.evaluator,
        )
        self.beam_width = beam_width
        self.max_epoch_depth = max_epoch_depth
        self.top_k_combat_evals = top_k_combat_evals
        self.refresh_samples = refresh_samples
        self.rng = random.Random(seed)

        self._queued_actions: Dict[int, List[QueuedStep]] = {}
        self._positioned_turn: Dict[int, Tuple[int, int]] = {}  # player -> (lobby_id, turn)
        self._stoch_hp_cache: Dict[Tuple[int, int, str, Optional[int]], bool] = {}
        self.last_note: Optional[Dict[str, float]] = None

    @staticmethod
    def _find_matching_legal(
        legal: Sequence[seaglass.TavernAction],
        target_key: ActionKey,
    ) -> Optional[seaglass.TavernAction]:
        for act in legal:
            if _action_key(act) == target_key:
                return act
        return None

    @staticmethod
    def _can_productive_refresh(obs: seaglass.TavernObservation) -> bool:
        if obs.free_refreshes > 0:
            return obs.gold >= 1 or obs.minion_cost == 0
        return obs.gold >= obs.refresh_cost + min(2, max(1, obs.minion_cost))

    def _is_stochastic_hero_power(
        self,
        g: seaglass.TavernGame,
        obs: seaglass.TavernObservation,
        act: seaglass.TavernAction,
    ) -> bool:
        if act.kind not in ("hero_power", "hero_power_2"):
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

    def _play_newly_gained_card_in_sim(self, sim: seaglass.TavernGame) -> None:
        """Helper for Discover/Trinket rollouts: play newly discovered card onto board/cycle slot."""
        obs_after = sim.observe()
        if obs_after.discover_pending is not None or obs_after.trinket_options or not obs_after.hand:
            return
        new_idx = len(obs_after.hand) - 1
        new_card = obs_after.hand[new_idx]
        if new_card.unplayable:
            return
        if new_card.is_spell:
            if new_card.card_id in (self.kb.robust_evolution_id, 847, 848, 863):
                return
            fast_cast = seaglass.TavernAction.play(new_idx, 0)
            if sim.is_legal(fast_cast):
                sim.step(fast_cast)
                return
            for sub in sim.legal_actions():
                if sub.kind == "play" and sub.args[0] == new_idx:
                    sim.step(sub)
                    break
            return

        tribe_counts: Dict[str, int] = {}
        for u in obs_after.board:
            for tr in u.tribes:
                tribe_counts[tr] = tribe_counts.get(tr, 0) + 1

        if len(obs_after.board) < 7:
            best_pos = len(obs_after.board)
            if new_card.card_id == self.kb.faceless_manipulator_id and obs_after.board:
                # `Faceless Manipulator` CAN copy Golden minions and Tier 7 minions!
                copy_cands = [
                    (
                        (18.0 if u.is_golden else 0.0)
                        + self.evaluator.effective_engine_bonus(
                            u.card_id, obs_after.turn, tribe_counts, on_board=True
                        )
                        + 3.5 * u.tavern_tier
                        + 0.12 * (u.attack + u.health)
                        + (10.0 if u.venomous else 0.0),
                        idx,
                    )
                    for idx, u in enumerate(obs_after.board)
                ]
                best_pos = max(copy_cands, key=lambda x: x[0])[1]
            elif new_card.card_id in self.kb.targeted_battlecry_ids and obs_after.board:
                non_gold = [
                    (
                        self.evaluator.effective_engine_bonus(
                            u.card_id, obs_after.turn, tribe_counts, on_board=True
                        )
                        + 3 * u.tavern_tier
                        + 0.1 * (u.attack + u.health),
                        idx,
                    )
                    for idx, u in enumerate(obs_after.board)
                    if not u.is_golden and u.tavern_tier <= 6
                ]
                if non_gold:
                    best_pos = max(non_gold, key=lambda x: x[0])[1]
            play_act = seaglass.TavernAction.play(new_idx, best_pos)
            if sim.is_legal(play_act):
                sim.step(play_act)
        elif len(obs_after.board) == 7 and not self.evaluator.has_hand_buff_synergy(obs_after):
            weak = expendable_slots(obs_after, self.evaluator, max_slots=1)
            if weak:
                w_slot = weak[0]
                w_unit = obs_after.board[w_slot]
                new_eng = self.evaluator.effective_engine_bonus(
                    new_card.card_id, obs_after.turn, tribe_counts, on_board=False
                )
                w_eng = self.evaluator.effective_engine_bonus(
                    w_unit.card_id,
                    obs_after.turn,
                    tribe_counts,
                    on_board=True,
                    board_units=obs_after.board,
                    is_golden=w_unit.is_golden,
                )
                w_scam = w_unit.venomous or w_unit.card_id in (self.kb.leeroy_id, self.kb.morgl_id)
                if not w_scam and (
                    new_card.is_golden
                    or new_card.card_id == self.kb.faceless_manipulator_id
                    or new_eng > w_eng + 3.0
                    or (new_card.tavern_tier > w_unit.tavern_tier and w_eng < 10.0)
                    or ((new_card.attack + new_card.health) > (w_unit.attack + w_unit.health + 4) and w_eng < 10.0)
                ):
                    sell_act = seaglass.TavernAction.sell(w_slot)
                    if sim.is_legal(sell_act):
                        sim.step(sell_act)
                        obs_mid = sim.observe()
                        if obs_mid.hand:
                            ins_p = min(w_slot, len(obs_mid.board))
                            if new_card.card_id == self.kb.faceless_manipulator_id and obs_mid.board:
                                ins_p = max(
                                    range(len(obs_mid.board)),
                                    key=lambda idx: (
                                        obs_mid.board[idx].is_golden,
                                        obs_mid.board[idx].tavern_tier,
                                        obs_mid.board[idx].attack + obs_mid.board[idx].health,
                                    ),
                                )
                            p_act = seaglass.TavernAction.play(
                                len(obs_mid.hand) - 1, ins_p
                            )
                            if sim.is_legal(p_act):
                                sim.step(p_act)

    def _evaluate_discover_options(
        self,
        base_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        particles: ApexOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
    ) -> seaglass.TavernAction:
        best_act = legal[0]
        best_val = -1e18
        for act in legal:
            sim = base_game.copy()
            sim.reseed(self.rng.getrandbits(63) | 1)
            sim.step(act)
            if sim.observe().discover_pending is not None:
                sub_l = sim.legal_actions()
                if sub_l:
                    sim.step(sub_l[0])
            self._play_newly_gained_card_in_sim(sim)
            val = self.evaluator.evaluate_turn_end(
                sim, particles, ctx=ctx, combats_per_particle=8
            )
            if val > best_val:
                best_val = val
                best_act = act
        return best_act

    def _choose_best_trinket(
        self,
        base_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        particles: ApexOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
    ) -> seaglass.TavernAction:
        """
        2-Turn Exact Rust Physics Rollout for Turn 6 & Turn 9 Trinkets!
        Simulates Turn `t` AND Turn `t + 1` in Rust (including `Guiding Candle` Tier-6 shop Refresh
        and context-aware `Safety Patch` / `Ironforge Anvil` checks) so multi-turn/compounding trinkets
        materialize their true compounding physics!
        """
        obs0 = base_game.observe()
        ehp0 = self.evaluator.effective_hp(obs0)
        alive_units = ctx.alive_units if ctx is not None else 8
        owns_souvenir = self.kb.souvenir_stand_id > 0 and any(
            t == self.kb.souvenir_stand_id for t in obs0.trinkets
        )
        avg_board_stats = (
            sum(u.attack + u.health for u in obs0.board) / len(obs0.board)
            if obs0.board
            else 0.0
        )
        no_type_cnt = sum(1 for u in obs0.board if len(u.tribes) == 0)
        no_type_stats = sum(u.attack + u.health for u in obs0.board if len(u.tribes) == 0)
        has_high_tier_carry = any(
            self.kb.engine_bonus_by_id.get(u.card_id, 0.0) >= 12.0 and u.tavern_tier >= 5
            for u in obs0.board
        )
        aberration_cnt = sum(1 for u in obs0.board if "aberration" in u.tribes or "all" in u.tribes)
        dr_cnt = sum(
            1
            for u in obs0.board
            if (self.meta.profile(u.card_id) is not None and self.meta.profile(u.card_id).has_deathrattle)
            or u.extra_deathrattles
        )
        stackable_greater_names = {
            "Weighted Gauntlet",
            "Tableware Portrait",
            "Wax Lance",
            "Ghastly Sticker",
            "Morgl Portrait",
            "Pagle's Fishing Rod",
            "Innkeeper's Hearth",
            "Bronzebeard Portrait",
            "Aggem Sticker",
            "Gold Mallet",
            "Murky Sticker",
            "Sphere of Memory",
            "Wolfhead Flail",
        }

        best_act = legal[0]
        best_val = -1e18

        for act in legal:
            if act.kind != "choose_trinket":
                continue
            opt_idx, _ = act.args
            t_prior = 0.0
            tid = -1
            if opt_idx is not None and opt_idx < len(obs0.trinket_options):
                tid = obs0.trinket_options[opt_idx]
                tinfo = self.kb.trinket_info.get(tid)
                if tinfo is not None:
                    tname, _is_g, _tcost, impl = tinfo
                    if not impl:
                        t_prior -= 12.0
                    t_prior += 0.72 * TRINKET_EQUITY_PRIORS.get(tname, 3.5)
                    if tname == "Weighted Gauntlet":
                        t_prior += 35.0
                    elif tname in ("Souvenir Stand", "Worn Treasure Map", "Stuffed Coin Purse", "Trip Vouchers"):
                        if ehp0 <= 18.0 or (alive_units <= 2 and obs0.turn <= 6):
                            t_prior -= 18.0
                    elif tname == "Mysterious Orb" and alive_units <= 2 and obs0.turn <= 6:
                        t_prior += 18.0
                    elif tname == "Shrine of Evolution" and avg_board_stats >= 10.0:
                        t_prior -= 24.0
                    elif tname == "Safety Patch" and ehp0 <= 15.0:
                        t_prior += 14.0
                    elif tname == "Ironforge Anvil":
                        if no_type_stats < 28:
                            t_prior -= 24.0
                        elif no_type_cnt >= 2:
                            t_prior += 5.5 * no_type_cnt
                    elif tname == "Maldraxxus Dagger":
                        if has_high_tier_carry and ehp0 >= 26.0:
                            t_prior += 6.0
                        else:
                            t_prior -= 22.0
                    elif tname in ("Mask of Ancient Ones", "Evil Experiment"):
                        if obs0.deity == "none" and aberration_cnt < 2:
                            t_prior -= 25.0
                    elif tname == "Soul Fermenter" and dr_cnt < 3:
                        t_prior -= 18.0
                    if owns_souvenir and _is_g:
                        if tname in stackable_greater_names:
                            t_prior += 12.0
                        elif tname in ("Manipulator Portrait", "Safety Patch"):
                            t_prior -= 8.0

            sim = base_game.copy()
            r_seed = self.rng.getrandbits(63) | 1
            sim.reseed(r_seed)
            sim.step(act)

            for _ in range(3):
                s_obs = sim.observe()
                if s_obs.discover_pending is not None:
                    d_legal = sim.legal_actions()
                    if d_legal:
                        pick = self._evaluate_discover_options(sim, d_legal, particles, ctx)
                        sim.step(pick)
                        self._play_newly_gained_card_in_sim(sim)
                elif s_obs.trinket_options:
                    t_legal = sim.legal_actions()
                    if t_legal:
                        sim.step(t_legal[0])
                else:
                    break

            self._play_newly_gained_card_in_sim(sim)
            if tid == self.kb.guiding_candle_id and sim.observe().gold >= 4:
                ref0 = seaglass.TavernAction.refresh()
                if sim.is_legal(ref0):
                    sim.step(ref0)

            sim_t0 = self._greedy_roll_in_copy(sim, ctx, particles=particles, max_steps=4)
            v_turn0 = self.evaluator.evaluate_turn_end(
                sim_t0, particles, ctx=ctx, combats_per_particle=10
            )

            v_turn1_macro = 0.0
            if particles.boards:
                sim_t1 = sim_t0.copy()
                c_seed = (obs0.turn * 262147 + (opt_idx or 0) * 997 + 5) | 1
                sim_t1.reseed(c_seed)
                if sim_t1.is_legal(seaglass.TavernAction.end_turn()):
                    sim_t1.end_turn()
                sim_t1.resolve_combat_against(
                    particles.boards[0], opponent_tier=particles.tier, seed=c_seed
                )
                sim_t1.start_turn()
                for _ in range(2):
                    if sim_t1.observe().discover_pending is not None:
                        dl = sim_t1.legal_actions()
                        if dl:
                            sim_t1.step(dl[0])
                    else:
                        break
                # `Guiding Candle` fills the first 2 Refreshes each turn with ONLY Tier 6 cards!
                if tid == self.kb.guiding_candle_id:
                    ref1 = seaglass.TavernAction.refresh()
                    if sim_t1.is_legal(ref1):
                        sim_t1.step(ref1)
                sim_t1 = self._greedy_roll_in_copy(sim_t1, ctx, particles=particles, max_steps=4)
                if sim_t1.is_legal(seaglass.TavernAction.end_turn()):
                    sim_t1.end_turn()
                sim_t1.resolve_combat_against(
                    particles.boards[0], opponent_tier=particles.tier, seed=c_seed + 2
                )
                sim_t1.start_turn()
                v_turn1_macro = self.evaluator.evaluate_fast(
                    sim_t1.observe(),
                    ctx=ctx,
                    end_of_turn=False,
                    particles=particles,
                    in_dist_sot=True,
                )

            total_val = 0.70 * v_turn0 + 0.30 * v_turn1_macro + t_prior
            if total_val > best_val:
                best_val = total_val
                best_act = act

        return best_act

    def _expand_cycle_macros(
        self,
        g: seaglass.TavernGame,
        obs: seaglass.TavernObservation,
        path: List[QueuedStep],
        particles: Optional[ApexOpponentParticles] = None,
    ) -> List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]]:
        """
        Compound `Sell(weak_slot) -> Play(hand_idx, ins_pos)` macros on a 7-minion board:
          - Supports replacing weak/filler slots with higher-tier compounding engines, Goldens,
            Venomous/Leeroy/Morgl tech, Battlecry buffs, targeted Battlecries, and 10-card hand unclogging.
        """
        if len(obs.board) < 7 or not obs.hand:
            return []
        weak_slots = expendable_slots(obs, self.evaluator, max_slots=2, particles=particles)
        if not weak_slots:
            return []

        tribe_counts: Dict[str, int] = {}
        scam_count = 0
        for u in obs.board:
            if u.venomous or u.card_id in (self.kb.leeroy_id, self.kb.morgl_id):
                scam_count += 1
            for tr in u.tribes:
                tribe_counts[tr] = tribe_counts.get(tr, 0) + 1

        owns_gauntlet = any(tid in self.kb.weighted_gauntlet_ids for tid in obs.trinkets)
        eff_colossus = (
            max(particles.colossus_count, particles.lobby_colossus_count)
            if (particles is not None and not owns_gauntlet)
            else 0
        )
        enemy_giant = particles.has_giant_unit if particles is not None else False
        enemy_sum = particles.has_summoners if particles is not None else False
        enemy_ds = particles.has_divine_shields if particles is not None else False
        drakkari_id = self.kb.name_to_id.get("Drakkari Enchanter", -1)
        hand_synergy = self.evaluator.has_hand_buff_synergy(obs)
        hand_clogged = (
            len(obs.hand) >= 9
            and not hand_synergy
            and not any(c.is_spell and not c.unplayable for c in obs.hand)
        )
        cycle_cap = max(32, int(3.4 * obs.turn))
        results: List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []
        pre_sig = _plan_validation_key(obs)

        for w_rank, w_slot in enumerate(weak_slots):
            w_unit = obs.board[w_slot]
            w_stats = w_unit.attack + w_unit.health
            w_eng = self.evaluator.effective_engine_bonus(
                w_unit.card_id,
                obs.turn,
                tribe_counts,
                on_board=True,
                board_units=obs.board,
                is_golden=w_unit.is_golden,
                obs_trinkets=obs.trinkets,
            )
            is_true_sell = (
                w_unit.card_id in self.kb.true_on_sell_plain_ids
                or (w_unit.card_id in self.kb.golden_only_sell_ids and w_unit.is_golden)
            )
            w_is_scam = w_unit.venomous or w_unit.card_id in (self.kb.leeroy_id, self.kb.morgl_id)
            is_cycle_slot = (w_stats < cycle_cap and w_eng < 10.0 and not w_is_scam) or is_true_sell
            if w_rank > 0 and (w_eng >= 10.0 or w_is_scam):
                continue

            sell_act = seaglass.TavernAction.sell(w_slot)
            if not g.is_legal(sell_act):
                continue
            w_keep = self.evaluator.unit_keep_priority(
                w_unit,
                obs.turn,
                tribe_counts,
                scam_count=scam_count,
                particles=particles,
                board_units=obs.board,
                obs_trinkets=obs.trinkets,
            )

            seen_macro_hand: set[Tuple] = set()
            for h_idx, h_card in enumerate(obs.hand):
                if h_card.is_spell or h_card.unplayable:
                    continue
                m_sym = (
                    h_card.card_id,
                    h_card.is_golden,
                    h_card.attack,
                    h_card.health,
                    h_card.counter,
                    tuple(h_card.dark_gifts),
                    (h_idx == 0) if hand_synergy else False,
                )
                if m_sym in seen_macro_hand:
                    continue
                seen_macro_hand.add(m_sym)
                h_prof = self.meta.profiles.get(h_card.card_id)
                h_stats = h_card.attack + h_card.health
                h_eng = self.evaluator.effective_engine_bonus(
                    h_card.card_id,
                    obs.turn,
                    tribe_counts,
                    on_board=False,
                    board_units=obs.board,
                    is_golden=h_card.is_golden,
                    obs_trinkets=obs.trinkets,
                )
                h_val = self.evaluator.unit_value(
                    h_card,
                    obs.turn,
                    tribe_counts,
                    enemy_has_giant=enemy_giant,
                    enemy_has_summoners=enemy_sum,
                    on_board=False,
                    colossus_count=eff_colossus,
                    enemy_has_ds=enemy_ds,
                    board_units=obs.board,
                    obs_trinkets=obs.trinkets,
                )
                is_merged_gold = h_card.is_golden and not (
                    h_prof is not None and h_prof.is_intrinsic_golden
                )
                scam_stat_cap = 450 if eff_colossus >= 3 else 260
                can_replace = (
                    (w_rank == 0 and is_cycle_slot)
                    or (h_val > w_keep + 1.5 and w_eng < 11.5 and not w_is_scam)
                    or (h_stats >= 0.72 * w_stats and w_eng < 10.0 and not w_is_scam)
                    or (is_merged_gold and w_stats < 75 and w_eng < 12.0 and not w_is_scam)
                    or (h_eng >= 10.0 and h_eng > w_eng + 3.0 and w_stats < 105 and not w_is_scam)
                    or (owns_gauntlet and h_card.card_id == drakkari_id and w_unit.card_id != drakkari_id)
                    or (
                        not w_is_scam
                        and (
                            h_card.venomous
                            or h_card.card_id in (self.kb.leeroy_id, self.kb.morgl_id, self.kb.sindorei_id)
                        )
                        and w_stats < scam_stat_cap
                        and (w_eng < 12.0 or eff_colossus >= 3)
                    )
                    or (
                        (
                            h_card.card_id == self.kb.tyrael_id
                            or h_card.card_id in self.kb.targeted_battlecry_ids
                        )
                        and w_eng < 10.0
                        and not w_is_scam
                    )
                    or (
                        w_rank == 0
                        and hand_clogged
                        and obs.gold >= 5
                        and not w_unit.divine_shield
                        and not w_is_scam
                        and w_unit.card_id != drakkari_id
                        and (h_card.tavern_tier >= 5 or h_eng >= 9.0)
                    )
                )
                if not can_replace:
                    continue

                mid = g.copy()
                mid.reseed(self.rng.getrandbits(63) | 1)
                mid.step(sell_act)
                mid_obs = mid.observe()
                if mid_obs.discover_pending is not None or h_idx >= len(mid_obs.hand):
                    continue

                if h_card.card_id in self.kb.targeted_battlecry_ids:
                    candidate_positions = list(range(len(mid_obs.board) + 1))
                else:
                    candidate_positions = [min(w_slot, len(mid_obs.board))]

                mid_sig = _plan_validation_key(mid_obs)
                step1: QueuedStep = (_action_key(sell_act), pre_sig, False)

                for ins_pos in candidate_positions:
                    play_act = seaglass.TavernAction.play(h_idx, ins_pos)
                    if not mid.is_legal(play_act):
                        continue
                    after_g = mid.copy()
                    after_g.reseed(self.rng.getrandbits(63) | 1)
                    after_g.step(play_act)
                    after_obs = after_g.observe()
                    gen_card = (
                        len(after_obs.hand) >= len(mid_obs.hand)
                        or after_obs.discover_pending is not None
                    )
                    step2: QueuedStep = (_action_key(play_act), mid_sig, gen_card)
                    results.append((after_g, after_obs, path + [step1, step2]))

        return results

    def _search_shop_epoch(
        self,
        root_game: seaglass.TavernGame,
        particles: ApexOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
    ) -> Tuple[float, List[QueuedStep], List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]]]:
        """
        Continuation-Aware DAG Beam Search within the current visible shop:
          Returns:
            - `best_eot_score`: Best full 1-turn physics score among `EndTurn` candidates,
            - `best_eot_path`: Action queue for that `EndTurn` plan,
            - `top_roll_prefixes`: Top non-empty prefixes `(fast_score, game, obs, path)` that
              bought high-value cards/upgrades from this shop and still have gold to `Refresh`!
        """
        root_obs = root_game.observe()
        root_fast = self.evaluator.evaluate_fast(
            root_obs, ctx=ctx, end_of_turn=False, particles=particles
        )
        transposition_best: Dict[Tuple, float] = {_canonical_state_key(root_obs): root_fast}

        beam: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = [
            (root_fast, root_game, root_obs, [])
        ]

        raw_terminals: List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, bool, List[QueuedStep]]] = []
        roll_prefixes: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []

        zero_cost_spells = sum(
            1 for c in root_obs.hand if c.is_spell and c.spell_cost == 0 and not c.unplayable
        )
        eff_depth = min(15, self.max_epoch_depth + zero_cost_spells)
        for _depth in range(eff_depth):
            raw_children: List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []

            for b_fast, g, obs, path in beam:
                obs_sig = _plan_validation_key(obs)
                end_step: QueuedStep = (("end_turn", None, None), obs_sig, False)
                raw_terminals.append((g, obs, True, path + [end_step]))

                # Record non-empty prefixes that still have productive Refresh budget!
                if path and obs.discover_pending is None and not obs.trinket_options and self._can_productive_refresh(obs):
                    roll_prefixes.append((b_fast, g, obs, path))

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

                legal = prune_apex_actions(
                    obs, g.legal_actions(), self.evaluator, self.kb, particles=particles
                )
                n_hand_pre = len(obs.hand)
                n_shop_pre = len(obs.shop)
                for act in legal:
                    k = act.kind
                    if k in ("end_turn", "toggle_freeze", "refresh", "dark_discovery", "pass"):
                        continue
                    if k in ("hero_power", "hero_power_2") and self._is_stochastic_hero_power(g, obs, act):
                        continue

                    child = g.copy()
                    child.reseed(self.rng.getrandbits(63) | 1)
                    ended = child.step(act)
                    c_obs = child.observe()

                    is_stoch_step = False
                    if (
                        k in ("play", "activate", "sell", "play_on_shop")
                        and c_obs.discover_pending is None
                    ):
                        # Fast filter: run secondary RNG probe when a card entered hand, shop mutated, or board transformed!
                        gained_hand_card = (
                            len(c_obs.hand) >= n_hand_pre
                            if k in ("play", "play_on_shop")
                            else len(c_obs.hand) > n_hand_pre
                        )
                        shop_mutated = (
                            len(c_obs.shop) != n_shop_pre and k != "play_on_shop"
                        )
                        board_transformed = (
                            k != "sell"
                            and len(c_obs.board) == len(obs.board)
                            and [u.card_id for u in c_obs.board] != [u.card_id for u in obs.board]
                        )
                        if gained_hand_card or shop_mutated or board_transformed:
                            probe = g.copy()
                            probe.reseed(999983)
                            probe.step(act)
                            if _decision_skeleton(probe.observe()) != _decision_skeleton(c_obs):
                                is_stoch_step = True

                    q_step: QueuedStep = (_action_key(act), obs_sig, is_stoch_step)
                    if ended:
                        raw_terminals.append((child, c_obs, True, path + [q_step]))
                    elif is_stoch_step:
                        raw_terminals.append((child, c_obs, False, path + [q_step]))
                    else:
                        raw_children.append((child, c_obs, path + [q_step]))

                raw_children.extend(self._expand_cycle_macros(g, obs, path, particles=particles))

            if not raw_children:
                break

            dedup_children: List[Tuple[Tuple, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []
            seen_layer: set[Tuple] = set()
            for child, c_obs, c_path in raw_children:
                ckey = _canonical_state_key(c_obs)
                if ckey in seen_layer:
                    continue
                seen_layer.add(ckey)
                dedup_children.append((ckey, child, c_obs, c_path))

            child_scores = self.evaluator.evaluate_fast_batch(
                [item[2] for item in dedup_children],
                ctx=ctx,
                end_of_turn=False,
                particles=particles,
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

        # Score all terminal candidates in batched calls
        eot_indices = [i for i, item in enumerate(raw_terminals) if item[2]]
        open_indices = [i for i, item in enumerate(raw_terminals) if not item[2]]
        term_scores = [0.0] * len(raw_terminals)

        if eot_indices:
            vals = self.evaluator.evaluate_fast_batch(
                [raw_terminals[i][1] for i in eot_indices],
                ctx=ctx,
                end_of_turn=True,
                particles=particles,
            )
            for idx, v in zip(eot_indices, vals):
                term_scores[idx] = v
        if open_indices:
            vals = self.evaluator.evaluate_fast_batch(
                [raw_terminals[i][1] for i in open_indices],
                ctx=ctx,
                end_of_turn=False,
                particles=particles,
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
            eval_game = cand_game
            cand_obs = cand_game.observe()
            if cand_obs.discover_pending is not None:
                eval_game = cand_game.copy()
                eval_game.reseed(self.rng.getrandbits(63) | 1)
                d_legal = eval_game.legal_actions()
                if d_legal:
                    d_pick = self._evaluate_discover_options(eval_game, d_legal, particles, ctx)
                    eval_game.step(d_pick)
                    self._play_newly_gained_card_in_sim(eval_game)
                eval_game = self._greedy_roll_in_copy(eval_game, ctx, particles=particles, max_steps=3)
            elif cand_path and cand_path[-1][2]:
                eval_game = self._greedy_roll_in_copy(cand_game, ctx, particles=particles, max_steps=3)

            full_score = self.evaluator.evaluate_turn_end(
                eval_game,
                particles,
                ctx=ctx,
                combats_per_particle=12,
            )
            if full_score > best_score:
                best_score = full_score
                best_path = cand_path

        # Deduplicate & rank non-empty prefixes that strictly improved upon `root_fast`
        # (i.e. bought/played genuinely positive-EV cards from this shop and still have roll gold!).
        roll_prefixes.sort(key=lambda x: x[0], reverse=True)
        top_prefixes: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []
        seen_pref: set[Tuple] = set()
        for p_fast, p_g, p_obs, p_path in roll_prefixes:
            if p_fast < root_fast - 0.5:
                continue
            pk = _canonical_state_key(p_obs)
            if pk in seen_pref:
                continue
            seen_pref.add(pk)
            top_prefixes.append((p_fast, p_g, p_obs, p_path))
            if len(top_prefixes) >= 2:
                break

        return best_score, best_path, top_prefixes

    def _greedy_roll_in_copy(
        self,
        start_game: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        particles: Optional[ApexOpponentParticles] = None,
        max_steps: int = 4,
    ) -> seaglass.TavernGame:
        """Fast multi-step greedy shop consumer after a stochastic Refresh / Discover."""
        sim = start_game.copy()
        sim.reseed(self.rng.getrandbits(63) | 1)
        for _step in range(max_steps):
            obs = sim.observe()
            if obs.discover_pending is not None:
                d_acts = sim.legal_actions()
                if d_acts:
                    # Pick highest-tier / highest-engine discover option quickly
                    best_d = d_acts[0]
                    best_dv = -1e9
                    for da in d_acts:
                        probe = sim.copy()
                        probe.step(da)
                        po = probe.observe()
                        dv = self.evaluator.evaluate_fast(
                            po, ctx=ctx, end_of_turn=False, particles=particles
                        )
                        if dv > best_dv:
                            best_dv = dv
                            best_d = da
                    sim.step(best_d)
                    self._play_newly_gained_card_in_sim(sim)
                continue

            cur_eot = self.evaluator.evaluate_fast(
                obs, ctx=ctx, end_of_turn=True, particles=particles
            )
            cur_total = cur_eot + 1.35 * obs.gold + 0.85 * obs.free_refreshes
            cand_sims: List[seaglass.TavernGame] = []
            cand_obs: List[seaglass.TavernObservation] = []

            pruned = prune_apex_actions(
                obs, sim.legal_actions(), self.evaluator, self.kb, particles=particles
            )
            top_buy_slots: set[int] = set()
            if obs.shop:
                buy_slots = [
                    a.args[0]
                    for a in pruned
                    if a.kind == "buy" and a.args[0] is not None and a.args[0] < len(obs.shop)
                ]
                if len(buy_slots) <= 3:
                    top_buy_slots = set(buy_slots)
                else:
                    buy_slots.sort(
                        key=lambda si: (
                            obs.shop[si].is_spell,
                            obs.shop[si].tavern_tier,
                            obs.shop[si].attack + obs.shop[si].health,
                        ),
                        reverse=True,
                    )
                    top_buy_slots = set(buy_slots[:3])

            for act in pruned:
                if act.kind in ("end_turn", "refresh", "toggle_freeze", "pass", "dark_discovery"):
                    continue
                if act.kind in ("hero_power", "hero_power_2") and self._is_stochastic_hero_power(sim, obs, act):
                    continue
                c = sim.copy()
                c.reseed(self.rng.getrandbits(63) | 1)
                c.step(act)
                if act.kind == "buy" and act.args[0] in top_buy_slots:
                    c_play = c.copy()
                    self._play_newly_gained_card_in_sim(c_play)
                    cand_sims.append(c_play)
                    cand_obs.append(c_play.observe())
                cand_sims.append(c)
                cand_obs.append(c.observe())

            for c_game, c_o, _ in self._expand_cycle_macros(sim, obs, [], particles=particles):
                cand_sims.append(c_game)
                cand_obs.append(c_o)

            if not cand_sims:
                # If we still have >= 4 gold after buying nothing in this rolled shop, simulate 1 more roll!
                if _step <= 1 and self._can_productive_refresh(obs) and obs.gold >= 4:
                    ref = seaglass.TavernAction.refresh()
                    if sim.is_legal(ref):
                        sim.step(ref)
                        continue
                break

            eot_scores = self.evaluator.evaluate_fast_batch(
                cand_obs, ctx=ctx, end_of_turn=True, particles=particles
            )
            scores = [
                ev + 1.35 * co.gold + 0.85 * co.free_refreshes
                for ev, co in zip(eot_scores, cand_obs)
            ]
            best_idx = max(range(len(scores)), key=lambda i: scores[i])
            if scores[best_idx] > cur_total - 0.15:
                sim = cand_sims[best_idx]
            elif _step <= 1 and self._can_productive_refresh(obs) and obs.gold >= 4:
                ref = seaglass.TavernAction.refresh()
                if sim.is_legal(ref):
                    sim.step(ref)
                    continue
                break
            else:
                break
        return sim

    def _evaluate_stochastic_action(
        self,
        base_game: seaglass.TavernGame,
        stoch_act: seaglass.TavernAction,
        particles: ApexOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
        n_samples: Optional[int] = None,
    ) -> float:
        samples = n_samples if n_samples is not None else self.refresh_samples
        total = 0.0
        for _ in range(samples):
            sim = base_game.copy()
            sim.reseed(self.rng.getrandbits(63) | 1)
            sim.step(stoch_act)
            legal_after = sim.legal_actions()
            if legal_after and legal_after[0].kind == "choose_discover":
                pick = self._evaluate_discover_options(sim, legal_after, particles, ctx)
                sim.step(pick)
                self._play_newly_gained_card_in_sim(sim)

            sim = self._greedy_roll_in_copy(sim, ctx, particles=particles, max_steps=4)
            total += self.evaluator.evaluate_turn_end(
                sim, particles, ctx=ctx, combats_per_particle=8
            )
        return total / max(1, samples)

    def choose_action_for_tavern(
        self,
        belief_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        player: int = 0,
        ctx: Optional[seaglass.LobbyContext] = None,
        lobby_id: int = 0,
    ) -> seaglass.TavernAction:
        obs = belief_game.observe()
        self.last_note = None
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            return legal[0]

        if legal[0].kind == "choose_hero":
            return self.evaluator.choose_best_hero(obs, legal, base_game=belief_game)

        particles = self.belief.next_opponent_particles(
            obs.turn, player, ctx, lobby_id=lobby_id
        )

        if legal[0].kind == "choose_trinket":
            self._queued_actions.pop(player, None)
            best_t = self._choose_best_trinket(belief_game, legal, particles, ctx)
            self.last_note = {"trinket_rollout": 2.0}
            return best_t

        if legal[0].kind == "choose_discover":
            self._queued_actions.pop(player, None)
            best_d = self._evaluate_discover_options(belief_game, legal, particles, ctx)
            self.last_note = {"discover_eval": 1.0}
            return best_d

        # Check queued plan (e.g. if called directly via `choose_action_for_tavern`)
        queued = self._queued_actions.get(player)
        if queued:
            next_key, exp_sig, stop_after = queued.pop(0)
            if _plan_validation_key(obs) == exp_sig:
                if next_key[0] == "end_turn":
                    self._queued_actions.pop(player, None)
                    return self._finalize_positioning_or_end(
                        belief_game, legal, player, obs.turn, particles, ctx, lobby_id=lobby_id
                    )
                matched = self._find_matching_legal(legal, next_key)
                if matched is not None:
                    if stop_after or not queued:
                        self._queued_actions.pop(player, None)
                    self.last_note = {"queued_plan": float(len(queued) + 1)}
                    return matched
            self._queued_actions.pop(player, None)

        # 1. Search current shop epoch (both `EndTurn` paths AND `Buy(Good) -> Refresh` prefixes!)
        epoch_score, epoch_path, top_roll_prefixes = self._search_shop_epoch(
            belief_game, particles, ctx
        )

        best_plan_score = epoch_score
        best_plan_path: List[QueuedStep] = epoch_path
        best_immediate_act: Optional[seaglass.TavernAction] = None

        # 2. Evaluate `DarkDiscovery` (3 uses per game! Save in 8p capped lobbies; unleash tempo in 1v1 / uncapped / low-HP!)
        dd_act = seaglass.TavernAction.dark_discovery()
        ehp_now = self.evaluator.effective_hp(obs)
        alive_units = ctx.alive_units if ctx is not None else 8
        dmg_cap = ctx.damage_cap if ctx is not None else None
        allow_dd = (
            obs.tavern_tier >= 3
            or (obs.tavern_tier >= 2 and obs.turn >= 4)
            or (alive_units <= 2 and obs.turn >= 3)
            or dmg_cap is None
            or ehp_now <= 20.0
        )
        if allow_dd and belief_game.is_legal(dd_act):
            dd_score = self._evaluate_stochastic_action(
                belief_game, dd_act, particles, ctx
            )
            if dd_score > best_plan_score + 0.25:
                best_plan_score = dd_score
                best_immediate_act = dd_act

        # 3. Evaluate stochastic / shop-mutating `HeroPower` & `HeroPower2`
        for act in legal:
            if act.kind in ("hero_power", "hero_power_2") and self._is_stochastic_hero_power(
                belief_game, obs, act
            ):
                hp_score = self._evaluate_stochastic_action(
                    belief_game, act, particles, ctx
                )
                if hp_score > best_plan_score + 0.25:
                    best_plan_score = hp_score
                    best_immediate_act = act

        # 4. Evaluate root `Refresh` (roll immediately without buying anything in current shop)
        ref_act = seaglass.TavernAction.refresh()
        if belief_game.is_legal(ref_act) and self._can_productive_refresh(obs):
            ref_score = self._evaluate_stochastic_action(
                belief_game, ref_act, particles, ctx
            )
            if ref_score > best_plan_score + 0.25:
                best_plan_score = ref_score
                best_immediate_act = ref_act

        # 5. Evaluate `Buy/Play(Good Cards) -> Refresh` continuation prefixes!
        #    Cures the "buy 1 good card then spend 7g on garbage in same shop" failure mode!
        for _p_fast, pref_game, pref_obs, pref_path in top_roll_prefixes:
            pref_ref = seaglass.TavernAction.refresh()
            if pref_game.is_legal(pref_ref):
                pref_roll_score = self._evaluate_stochastic_action(
                    pref_game, pref_ref, particles, ctx, n_samples=max(3, self.refresh_samples - 1)
                )
                if pref_roll_score > best_plan_score + 0.25:
                    best_plan_score = pref_roll_score
                    best_immediate_act = None
                    ref_step: QueuedStep = (
                        ("refresh", None, None),
                        _plan_validation_key(pref_obs),
                        True,  # Stop queue right after Refresh so next epoch observes new shop!
                    )
                    best_plan_path = pref_path + [ref_step]

        self.last_note = {
            "value": round(float(best_plan_score), 3),
            "epoch_ev": round(float(epoch_score), 3),
        }

        if best_immediate_act is not None:
            self._queued_actions.pop(player, None)
            matched = self._find_matching_legal(legal, _action_key(best_immediate_act))
            if matched is not None:
                return matched

        if not best_plan_path or best_plan_path[0][0][0] == "end_turn":
            # Hard Zero-Waste Invariant: NEVER click EndTurn and burn unspent gold on an unfrozen shop!
            if (
                not obs.is_frozen
                and belief_game.is_legal(ref_act)
                and (
                    (obs.free_refreshes > 0 and obs.gold >= 1)
                    or (obs.turn >= 3 and obs.gold >= obs.refresh_cost + 2)
                )
            ):
                matched_ref = self._find_matching_legal(legal, _action_key(ref_act))
                if matched_ref is not None:
                    self._queued_actions.pop(player, None)
                    self.last_note = {"zero_waste_refresh": float(obs.gold)}
                    return matched_ref
            return self._finalize_positioning_or_end(
                belief_game, legal, player, obs.turn, particles, ctx, lobby_id=lobby_id
            )

        first_key, _, first_stop = best_plan_path[0]
        if len(best_plan_path) > 1 and not first_stop:
            trimmed: List[QueuedStep] = []
            for st in best_plan_path[1:]:
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
            belief_game, legal, player, obs.turn, particles, ctx, lobby_id=lobby_id
        )

    def _finalize_positioning_or_end(
        self,
        belief_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        player: int,
        turn: int,
        particles: ApexOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
        lobby_id: int = 0,
    ) -> seaglass.TavernAction:
        obs = belief_game.observe()

        # Hard Invariant: NEVER end the turn with < 7 minions on board while a beneficial
        # playable minion (or 0-cost Triple Reward / Coin) is sitting idle in hand!
        if len(obs.board) < 7 and obs.hand:
            has_open_minion = any(
                not c.is_spell and not c.unplayable for c in obs.hand
            )
            if has_open_minion:
                stop_val = self.evaluator.evaluate_fast(
                    obs, ctx=ctx, end_of_turn=True, particles=particles
                )
                best_play: Optional[seaglass.TavernAction] = None
                best_pval = stop_val + 0.5
                for la in legal:
                    if la.kind != "play":
                        continue
                    h_i, _ = la.args
                    if h_i is None or h_i >= len(obs.hand) or obs.hand[h_i].is_spell:
                        continue
                    probe = belief_game.copy()
                    probe.reseed(self.rng.getrandbits(63) | 1)
                    if probe.is_legal(la):
                        probe.step(la)
                        pv = self.evaluator.evaluate_fast(
                            probe.observe(), ctx=ctx, end_of_turn=True, particles=particles
                        )
                        if pv > best_pval:
                            best_pval = pv
                            best_play = la
                if best_play is not None:
                    self._queued_actions.pop(player, None)
                    self.last_note = {"rescue_open_play": 1.0}
                    return best_play

        # Hard Zero-Waste Invariant (applies to queued `EndTurn` plans too!):
        # NEVER click EndTurn and burn unspent roll+buy gold on an unfrozen shop!
        ref_act = seaglass.TavernAction.refresh()
        if (
            not obs.is_frozen
            and belief_game.is_legal(ref_act)
            and (
                (obs.free_refreshes > 0 and obs.gold >= 1)
                or (obs.turn >= 3 and obs.gold >= obs.refresh_cost + 2)
            )
        ):
            matched_ref = self._find_matching_legal(legal, _action_key(ref_act))
            if matched_ref is not None:
                self._queued_actions.pop(player, None)
                self.last_note = {"zero_waste_refresh": float(obs.gold)}
                return matched_ref

        pos_tag = (lobby_id, turn)
        if self._positioned_turn.get(player) != pos_tag and len(obs.board) >= 2:
            self._positioned_turn[player] = pos_tag
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

    @staticmethod
    def _dominant_tribe(units: Sequence[seaglass.Unit]) -> Tuple[Optional[str], int]:
        counts: Dict[str, int] = {}
        for u in units:
            for tr in u.tribes:
                if tr != "all":
                    counts[tr] = counts.get(tr, 0) + 1
        if not counts:
            return None, 0
        best_tr, cnt = max(counts.items(), key=lambda kv: kv[1])
        return (best_tr if cnt >= 2 else None), cnt

    def _check_duos_pass_action(
        self,
        obs: Any,
        legal: Sequence[seaglass.TavernAction],
        player: int,
    ) -> Optional[seaglass.TavernAction]:
        """High-IQ Duos coordination: Pass Triples, free synergy gifts, and carry engines."""
        if not hasattr(obs, "teammate_board") or not legal:
            return None
        if legal[0].kind in ("choose_hero", "choose_discover", "choose_trinket"):
            return None

        mate_units = (
            list(obs.teammate_board)
            + list(obs.teammate_hand)
            + list(obs.teammate_incoming)
        )
        mate_counts: Dict[int, int] = {}
        mate_has_pass_scaler = False
        passenger_id = self.kb.name_to_id.get("Passenger", -1)
        reactor_id = self.kb.name_to_id.get("Transport Reactor", -1)
        mantid_id = self.kb.name_to_id.get("Mantid King", -1)

        for u in mate_units:
            if u.card_id in (passenger_id, reactor_id, mantid_id):
                mate_has_pass_scaler = True
            if not u.is_golden and not u.is_spell:
                mate_counts[u.card_id] = mate_counts.get(u.card_id, 0) + 1

        own_counts: Dict[int, int] = {}
        for u in list(obs.board) + list(obs.hand):
            if not u.is_golden and not u.is_spell:
                own_counts[u.card_id] = own_counts.get(u.card_id, 0) + 1

        mate_tribe, mate_tr_cnt = self._dominant_tribe(obs.teammate_board)
        own_tribe, _ = self._dominant_tribe(obs.board)
        free_pass = getattr(obs, "free_passes", 0) > 0

        # 1. Pass from hand: complete teammate's Triple, free synergy pass, or paid Tier-5+ carry engine pass!
        for act in legal:
            if act.kind == "pass":
                h_idx, _ = act.args
                if h_idx is not None and h_idx < len(obs.hand):
                    card = obs.hand[h_idx]
                    if card.unplayable:
                        continue
                    completes_triple = (
                        not card.is_spell
                        and not card.is_golden
                        and mate_counts.get(card.card_id, 0) >= 2
                        and own_counts.get(card.card_id, 0) < 3
                    )
                    c_eng = self.kb.engine_bonus_by_id.get(card.card_id, 0.0)
                    free_synergy = (
                        free_pass
                        and not card.is_spell
                        and (
                            (
                                mate_tribe is not None
                                and mate_tribe != own_tribe
                                and mate_tribe in card.tribes
                                and (card.tavern_tier >= max(2, obs.teammate_tavern_tier - 1) or c_eng >= 8.0)
                            )
                            or (mate_has_pass_scaler and len(obs.board) >= 6 and c_eng == 0.0 and card.tavern_tier <= 2)
                        )
                    )
                    paid_engine_pass = (
                        not free_pass
                        and obs.gold >= 4
                        and len(obs.board) >= 6
                        and len(obs.teammate_hand) <= 7
                        and not card.is_spell
                        and c_eng >= 12.0
                        and mate_tribe is not None
                        and mate_tr_cnt >= 3
                        and mate_tribe != own_tribe
                        and mate_tribe in card.tribes
                    )
                    if completes_triple or free_synergy or paid_engine_pass:
                        self._queued_actions.pop(player, None)
                        self.last_note = {"duos_pass": 1.0}
                        return act

        # 2. Buy from shop specifically to Pass if it completes teammate's Triple!
        pass_cost = 0 if free_pass else 1
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
                            and (sc.tavern_tier >= 2 or obs.turn <= 7 or len(obs.teammate_board) < 7)
                        ):
                            self._queued_actions.pop(player, None)
                            self.last_note = {"duos_buy_for_pass": 1.0}
                            return act
        return None

    def act_in_lobby(
        self, lobby: seaglass.LobbyGame, player: int
    ) -> seaglass.TavernAction:
        legal = lobby.legal_actions(player)
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            return legal[0]

        obs = lobby.observe(player)
        lobby_id = id(lobby)

        if lobby.duos and self.catalog_name != "duos":
            self.catalog_name = "duos"
            self.kb = get_apex_knowledge("duos")
            self.meta = self.kb.meta
            self.belief = ApexBeliefTracker(
                catalog_name="duos",
                num_particles=self.belief.num_particles,
                seed=self.rng.getrandbits(31),
            )
            self.evaluator = ApexEvaluator(
                catalog_name="duos",
                use_physics_rollout=self.evaluator.use_physics_rollout,
                use_neural_value=self.evaluator.use_neural_value,
                neural_weight=self.evaluator.neural_weight,
            )
            self.positioner = ApexPositioner(
                catalog_name="duos",
                max_candidates=self.positioner.max_candidates,
                evaluator=self.evaluator,
            )

        if lobby.phase == "hero" or legal[0].kind == "choose_hero":
            self._queued_actions.pop(player, None)
            bg = lobby.belief_tavern(player, seed=self.rng.getrandbits(63) | 1, extra_units=[])
            return self.evaluator.choose_best_hero(obs, legal, base_game=bg)

        # Duos Pass check
        if lobby.duos:
            pass_act = self._check_duos_pass_action(obs, legal, player)
            if pass_act is not None:
                return pass_act

        # 0.02ms FAST PATH: Execute validated deterministic queued actions WITHOUT rebuilding `belief_tavern`!
        queued = self._queued_actions.get(player)
        if queued and legal[0].kind not in ("choose_discover", "choose_trinket"):
            next_key, exp_sig, stop_after = queued[0]
            if _plan_validation_key(obs) == exp_sig:
                if next_key[0] != "end_turn":
                    matched = self._find_matching_legal(legal, next_key)
                    if matched is not None:
                        queued.pop(0)
                        if stop_after or not queued:
                            self._queued_actions.pop(player, None)
                        self.last_note = {"queued_plan": float(len(queued) + 1)}
                        return matched
                elif (
                    (self._positioned_turn.get(player) == (lobby_id, obs.turn) or len(obs.board) < 2)
                    and not (
                        not obs.is_frozen
                        and (
                            (obs.free_refreshes > 0 and obs.gold >= 1)
                            or (obs.turn >= 3 and obs.gold >= obs.refresh_cost + 2)
                        )
                    )
                    and not (
                        len(obs.board) < 7
                        and any(not c.is_spell and not c.unplayable for c in obs.hand)
                    )
                ):
                    # Already positioned & zero unspent roll gold / open hand plays! Return `end_turn` in 0.02ms!
                    self._queued_actions.pop(player, None)
                    end_act = self._find_matching_legal(legal, ("end_turn", None, None))
                    if end_act is not None:
                        return end_act
            else:
                self._queued_actions.pop(player, None)

        ctx = lobby.context(player)
        depletion = self.belief.sample_pool_depletion_units(
            lobby.turn, ctx, player=player, lobby_id=lobby_id
        )
        belief_game = lobby.belief_tavern(
            player, seed=self.rng.getrandbits(63) | 1, extra_units=depletion
        )
        return self.choose_action_for_tavern(
            belief_game, legal, player=player, ctx=ctx, lobby_id=lobby_id
        )

    def _choose_action_from_obs_only(
        self,
        obs: seaglass.TavernObservation,
        legal: Sequence[seaglass.TavernAction],
    ) -> seaglass.TavernAction:
        """
        100% observation-only policy for recorded `seaglass.MatchPlayer` (`BattlegroundsEnv`),
        where no `.copy()`able `TavernGame` or `belief_tavern` handle exists on the seat.
        """
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            return legal[0]
        k0 = legal[0].kind
        if k0 == "choose_hero":
            return self.evaluator.choose_best_hero(obs, legal, base_game=None)
        if k0 == "choose_trinket":
            best_t = legal[0]
            best_tv = -1e18
            for a in legal:
                oi = a.args[0]
                if oi is not None and oi < len(obs.trinket_options):
                    tid = obs.trinket_options[oi]
                    tinfo = self.kb.trinket_info.get(tid)
                    tname = tinfo[0] if tinfo is not None else ""
                    tcost = tinfo[2] if tinfo is not None else 0
                    tv = TRINKET_EQUITY_PRIORS.get(tname, 3.5) - 0.8 * tcost
                    if tv > best_tv:
                        best_tv = tv
                        best_t = a
            return best_t
        if k0 == "choose_discover":
            best_d = legal[0]
            best_dv = -1e18
            d_opts = obs.discover_pending or []
            tribe_counts: Dict[str, int] = {}
            for u in obs.board:
                for tr in u.tribes:
                    tribe_counts[tr] = tribe_counts.get(tr, 0) + 1
            for a in legal:
                oi = a.args[0]
                if oi is not None and oi < len(d_opts):
                    du = d_opts[oi]
                    dv = (
                        (5.0 if not du.is_spell else 2.5)
                        + self.evaluator.unit_value(
                            du, obs.turn, tribe_counts, on_board=False, board_units=obs.board
                        )
                    )
                    if dv > best_dv:
                        best_dv = dv
                        best_d = a
            return best_d

        pruned = prune_apex_actions(obs, legal, self.evaluator, self.kb)
        tribe_counts = {}
        owned: Dict[int, int] = {}
        for u in obs.board:
            for tr in u.tribes:
                tribe_counts[tr] = tribe_counts.get(tr, 0) + 1
            if not u.is_golden and not u.is_spell:
                owned[u.card_id] = owned.get(u.card_id, 0) + 1
        for c in obs.hand:
            if not c.is_golden and not c.is_spell:
                owned[c.card_id] = owned.get(c.card_id, 0) + 1

        # 1. Play cards from hand when open board slots exist or spells are free/buffing
        for a in pruned:
            if a.kind == "play":
                hi = a.args[0]
                if hi is not None and hi < len(obs.hand):
                    hc = obs.hand[hi]
                    if hc.is_spell and not hc.unplayable:
                        return a
                    if not hc.is_spell and not hc.unplayable and len(obs.board) < 7:
                        return a

        # 2. Standard Turn 2 / cheap-upgrade curve when healthy
        ehp = self.evaluator.effective_hp(obs)
        if obs.tavern_tier < 6:
            want_upg = (
                (obs.turn == 2 and obs.tavern_tier == 1 and obs.gold >= obs.upgrade_cost)
                or (obs.turn >= 4 and obs.tavern_tier == 1 and obs.gold >= obs.upgrade_cost)
                or (obs.turn >= 5 and obs.tavern_tier == 2 and obs.upgrade_cost <= 5 and ehp >= 22)
                or (obs.turn >= 7 and obs.tavern_tier == 3 and obs.upgrade_cost <= 6 and ehp >= 22)
                or (obs.turn >= 9 and obs.tavern_tier == 4 and obs.upgrade_cost <= 7 and ehp >= 24)
                or (obs.turn >= 11 and obs.tavern_tier == 5 and obs.upgrade_cost <= 7 and ehp >= 26)
            )
            if want_upg:
                upg = self._find_matching_legal(pruned, ("upgrade", None, None))
                if upg is not None:
                    return upg

        # 3. Dark Discovery on Turn 4+
        if obs.turn >= 4:
            dd = self._find_matching_legal(pruned, ("dark_discovery", None, None))
            if dd is not None:
                return dd

        # 4. Buy best shop card
        best_buy = None
        best_bv = -1e18
        for a in pruned:
            if a.kind == "buy":
                si = a.args[0]
                if si is not None and si < len(obs.shop):
                    sc = obs.shop[si]
                    if sc.is_spell:
                        bv = 7.5 if sc.spell_cost <= 1 else 5.0
                    else:
                        bv = self.evaluator.unit_value(
                            sc, obs.turn, tribe_counts, on_board=False, board_units=obs.board
                        )
                        if owned.get(sc.card_id, 0) >= 2:
                            bv += 18.0
                    if bv > best_bv:
                        best_bv = bv
                        best_buy = a
        if best_buy is not None and (len(obs.board) < 7 or best_bv >= 14.0):
            return best_buy

        # 5. Sell weakest slot if holding playable upgrade in hand on full board
        if len(obs.board) >= 7 and any(not c.is_spell and not c.unplayable for c in obs.hand):
            for a in pruned:
                if a.kind == "sell":
                    return a

        # 6. Refresh if gold remains
        if self._can_productive_refresh(obs):
            ref = self._find_matching_legal(pruned, ("refresh", None, None))
            if ref is not None:
                return ref

        end_a = self._find_matching_legal(legal, ("end_turn", None, None))
        return end_a if end_a is not None else legal[0]

    def act(self, seat: Any) -> int:
        """
        `seaglass_rl` discrete action interface (`seat -> int` index in `seaglass_rl.actions.ACTIONS`).
        Supports:
          - `seaglass_rl.play.Seat` (`seat.game` is a `seaglass.LobbyGame`, `seat.player` is int),
          - Standalone `seaglass.TavernGame` (always copied & reseeded for zero hidden-RNG leakage),
          - `seaglass.MatchPlayer` (`BattlegroundsEnv.player` — observation-only policy)!
        """
        from seaglass_rl.actions import ACTION_TO_INDEX, END_TURN_INDEX

        if hasattr(seat, "game") and hasattr(seat, "player") and isinstance(seat.game, seaglass.LobbyGame):
            act = self.act_in_lobby(seat.game, int(seat.player))
            arg0, arg1 = act.args
            idx = ACTION_TO_INDEX.get((act.kind, arg0, arg1))
            if idx is not None and seat.is_legal(act):
                return idx
            for la in seat.legal_actions():
                a0, a1 = la.args
                li = ACTION_TO_INDEX.get((la.kind, a0, a1))
                if li is not None:
                    return li
            return END_TURN_INDEX

        if isinstance(seat, seaglass.TavernGame):
            legal = seat.legal_actions()
            belief_copy = seat.copy()
            belief_copy.reseed(self.rng.getrandbits(63) | 1)
            act = self.choose_action_for_tavern(
                belief_copy, legal, player=0, ctx=None, lobby_id=id(seat)
            )
            arg0, arg1 = act.args
            idx = ACTION_TO_INDEX.get((act.kind, arg0, arg1))
            if idx is not None and seat.is_legal(act):
                return idx
            for la in legal:
                a0, a1 = la.args
                li = ACTION_TO_INDEX.get((la.kind, a0, a1))
                if li is not None:
                    return li
            return END_TURN_INDEX

        if hasattr(seat, "observe") and hasattr(seat, "legal_actions"):
            legal = seat.legal_actions()
            act = self._choose_action_from_obs_only(seat.observe(), legal)
            arg0, arg1 = act.args
            idx = ACTION_TO_INDEX.get((act.kind, arg0, arg1))
            if idx is not None and seat.is_legal(act):
                return idx
            for la in legal:
                a0, a1 = la.args
                li = ACTION_TO_INDEX.get((la.kind, a0, a1))
                if li is not None:
                    return li
            return END_TURN_INDEX

        raise TypeError(f"Unsupported seat type for ApexBot.act: {type(seat)!r}")
