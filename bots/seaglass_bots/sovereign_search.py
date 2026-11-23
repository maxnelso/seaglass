"""SovereignBot: CRN Multi-Epoch Stratified Beam + Positioned Physics + 5-Tribe Conditioned 8p-FFA Bot."""

from __future__ import annotations

import inspect
import os
import random
from typing import Any, Dict, List, Optional, Sequence, Tuple

import seaglass

from .apex_knowledge import ApexKnowledge, get_apex_knowledge
from .sovereign_belief import SovereignBeliefTracker, SovereignOpponentParticles
from .sovereign_evaluator import SovereignEvaluator
from .sovereign_knowledge import SovereignKnowledge, get_sovereign_knowledge
from .sovereign_positioning import SovereignPositioner
from .sovereign_trace import DecisionTrace, GateTrace, PlanTrace, note_from_trace

ActionKey = Tuple[str, Optional[int], Optional[int]]
QueuedStep = Tuple[ActionKey, Tuple, bool]


def _action_key(a: seaglass.TavernAction) -> ActionKey:
    arg0, arg1 = a.args
    return (a.kind, arg0, arg1)


def _plan_validation_key(obs: seaglass.TavernObservation) -> Tuple:
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


def _protected_scam_slots(
    obs: seaglass.TavernObservation,
    kb: ApexKnowledge,
    particles: Optional[SovereignOpponentParticles] = None,
) -> set[int]:
    """
    Board indices of scam units that should NOT be sold/cycled:
      - `Morgl` or Venomous with Divine Shield / Reborn are always protected,
      - Plain `Deadly Spore` / `Leeroy` are protected up to `max_prot` based on enemy giant/colossus threat,
        so excess/midgame `1/1` Spores never permanently clog board slots!
    """
    scam_indices = [
        idx
        for idx, bu in enumerate(obs.board)
        if bu.venomous or bu.card_id in (kb.leeroy_id, kb.morgl_id)
    ]
    if not scam_indices:
        return set()

    always_prot = {
        idx
        for idx in scam_indices
        if obs.board[idx].card_id == kb.morgl_id
        or (obs.board[idx].venomous and (obs.board[idx].divine_shield or obs.board[idx].reborn))
    }
    owns_gauntlet = any(tid in kb.weighted_gauntlet_ids for tid in obs.trinkets)
    eff_col = (
        max(particles.colossus_count, particles.lobby_colossus_count)
        if (particles is not None and not owns_gauntlet)
        else 0
    )
    enemy_giant = bool(particles is not None and particles.has_giant_unit)
    max_prot = (
        4
        if eff_col >= 3
        else (2 if (eff_col >= 1 or enemy_giant) else (1 if obs.turn >= 13 else 0))
    )
    if max_prot <= len(always_prot):
        return always_prot
    ranked = sorted(
        scam_indices,
        key=lambda idx: (
            idx in always_prot,
            obs.board[idx].card_id == kb.leeroy_id,
            obs.board[idx].attack + obs.board[idx].health,
            idx,
        ),
        reverse=True,
    )
    return always_prot | set(ranked[:max_prot])


def _cycle_stat_cap(turn: int) -> int:
    if turn <= 5:
        return 12
    if turn <= 9:
        return 11 + 3 * (turn - 5)
    return max(32, int(3.4 * turn))


def expendable_slots(
    obs: seaglass.TavernObservation,
    evaluator: SovereignEvaluator,
    max_slots: int = 2,
    particles: Optional[SovereignOpponentParticles] = None,
    ctx: Optional[seaglass.LobbyContext] = None,
) -> List[int]:
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
    lobby_tribes = ctx.tribes if ctx is not None else None
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
                lobby_tribes=lobby_tribes,
            ),
            idx,
        )
        for idx, u in enumerate(obs.board)
    ]
    ranked.sort(key=lambda x: x[0])
    return [idx for _, idx in ranked[: min(n, max_slots)]]


def prune_sovereign_actions(
    obs: seaglass.TavernObservation,
    legal: Sequence[seaglass.TavernAction],
    evaluator: SovereignEvaluator,
    kb: ApexKnowledge,
    particles: Optional[SovereignOpponentParticles] = None,
    ctx: Optional[seaglass.LobbyContext] = None,
) -> List[seaglass.TavernAction]:
    meta = kb.meta
    out: List[seaglass.TavernAction] = []
    seen_play_minion_hand: Dict[int, bool] = {}
    lobby_tribes = ctx.tribes if ctx is not None else None

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

    weak_slots = expendable_slots(obs, evaluator, max_slots=2, particles=particles, ctx=ctx)
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
            lobby_tribes=lobby_tribes,
        )
        if weakest_unit is not None
        else 0.0
    )

    cycle_stat_cap = _cycle_stat_cap(obs.turn)
    weakest_is_on_sell = bool(
        weakest_unit is not None
        and (
            weakest_unit.card_id in kb.true_on_sell_plain_ids
            or (weakest_unit.card_id in kb.golden_only_sell_ids and weakest_unit.is_golden)
        )
    )
    prot_scam = _protected_scam_slots(obs, kb, particles)
    weakest_is_scam = weakest_idx in prot_scam
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

        if k == "upgrade":
            if ehp_now <= 13.0 and obs.upgrade_cost >= 4:
                continue
            if ehp_now <= 17.0 and obs.upgrade_cost >= 7:
                continue
            if obs.turn <= 8 and obs.tavern_tier >= 4 and obs.upgrade_cost >= 8 and ehp_now <= 26.0:
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
                lobby_tribes=lobby_tribes,
            )
            is_true_sell = (
                u_sell.card_id in kb.true_on_sell_plain_ids
                or (u_sell.card_id in kb.golden_only_sell_ids and u_sell.is_golden)
            )
            u_stats = u_sell.attack + u_sell.health
            u_is_scam = b_pos in prot_scam

            if len(obs.board) < 7:
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
                        or (
                            obs.gold + 1 == obs.upgrade_cost
                            and (obs.turn <= 5 or (ehp_now >= 26.0 and len(obs.board) >= 6))
                        )
                    )
                ):
                    out.append(act)
                continue

            hand_has_major_upgrade = any(
                not c.is_spell
                and not c.unplayable
                and (
                    c.is_golden
                    or evaluator.effective_engine_bonus(
                        c.card_id,
                        obs.turn,
                        tribe_counts,
                        on_board=False,
                        board_units=obs.board,
                        obs_trinkets=obs.trinkets,
                        lobby_tribes=lobby_tribes,
                    )
                    > max(9.0, weakest_eng + 3.0)
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
                if s_card.is_spell and s_card.spell_cost > 0:
                    if s_card.card_id == 816 and ehp_now <= 14.0:
                        continue
                    if s_card.card_id == 820 and ehp_now <= 16.0:
                        continue
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
                        lobby_tribes=lobby_tribes,
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
                        lobby_tribes=lobby_tribes,
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
                                lobby_tribes=lobby_tribes,
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
                                lobby_tribes=lobby_tribes,
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
                    is_targeted_bc = (
                        card.card_id in kb.targeted_battlecry_ids
                        or card.card_id not in meta.templates_by_id
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


class SovereignBot:
    """
    Sovereign Imperfect-Information 8-Player FFA Bot (`"sovereign"`):
      1. Positioned 1-Turn Physics in Search (`SovereignPositioner.fast_reposition_game` inside `evaluate_turn_end`),
      2. 5-Tribe Lobby Conditioning (`ctx.tribes`) across Hero Select, Neutral Multipliers, Trinkets & Neural Net,
      3. Stratified Multi-Prefix Continuation Search (`roll_continuation_bonus`, up to 4 stratified prefixes),
      4. Common Random Numbers (CRN) across competing stochastic actions + `M=2` CRN 2-turn Trinket rollouts,
      5. Fixed Refresh Rollout Consumer (minion-first sort + play-all-minions + Magnetic 7-board attachment)
         and Full-Turn Discover Continuation,
      6. Pure 8-Player FFA `SovereignValueNet` (`SovereignNumpyPredictor` ~0.045ms BLAS).
    """

    def __init__(
        self,
        catalog_name: str = "full",
        beam_width: int = 24,
        max_epoch_depth: int = 10,
        top_k_combat_evals: int = 8,
        refresh_samples: int = 6,
        num_particles: int = 4,
        seed: int = 2029,
        evaluator: Optional[SovereignEvaluator] = None,
        weights_path: Optional[os.PathLike | str] = None,
    ) -> None:
        self.catalog_name = catalog_name
        self.sov_kb: SovereignKnowledge = get_sovereign_knowledge(catalog_name)
        self.kb: ApexKnowledge = self.sov_kb.base
        self.meta = self.kb.meta
        self.belief = SovereignBeliefTracker(
            catalog_name=catalog_name, num_particles=num_particles, seed=seed
        )
        w_str = str(weights_path) if weights_path is not None else None
        self.evaluator = (
            evaluator
            if evaluator is not None
            else SovereignEvaluator(catalog_name=catalog_name, weights_path=w_str)
        )
        self.positioner: SovereignPositioner = self.evaluator.positioner
        self.beam_width = beam_width
        self.max_epoch_depth = max_epoch_depth
        self.top_k_combat_evals = top_k_combat_evals
        self.refresh_samples = refresh_samples
        self.rng = random.Random(seed)

        self._queued_actions: Dict[int, List[QueuedStep]] = {}
        self._positioned_turn: Dict[int, Tuple[int, int]] = {}
        self.last_note: Optional[Dict[str, float]] = None
        # Decision tracing (records only; never draws from `self.rng` / `self.belief.rng`).
        self.last_trace: Optional[DecisionTrace] = None
        self._trace: Optional[DecisionTrace] = None
        self._queue_meta: Dict[int, Tuple[Optional[float], str]] = {}

    def _tracing(self) -> Optional[DecisionTrace]:
        """The live trace, if the evaluator supports score breakdowns (subclasses may not)."""
        tr = self._trace
        if tr is None:
            return None
        try:
            ok = "parts" in inspect.signature(self.evaluator.evaluate_turn_end).parameters
        except (TypeError, ValueError):
            ok = False
        return tr if ok else None

    @staticmethod
    def _step_keys(path: Sequence[QueuedStep]) -> List[ActionKey]:
        return [st[0] for st in path]

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

    def _fast_resolve_chained_discover(
        self,
        sim: seaglass.TavernGame,
        particles: Optional[SovereignOpponentParticles] = None,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> None:
        for _ in range(2):
            obs = sim.observe()
            if obs.discover_pending is None:
                break
            d_acts = sim.legal_actions()
            if not d_acts:
                break
            best_d = d_acts[0]
            best_dv = -1e18
            for da in d_acts:
                probe = sim.copy()
                probe.step(da)
                dv = self.evaluator.evaluate_fast(
                    probe.observe(), ctx=ctx, end_of_turn=False, particles=particles
                )
                if dv > best_dv:
                    best_dv = dv
                    best_d = da
            sim.step(best_d)

    def _play_newly_gained_card_in_sim(
        self,
        sim: seaglass.TavernGame,
        particles: Optional[SovereignOpponentParticles] = None,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> None:
        """
        Upgraded rollout card player:
          - Resolves Triple/Discover prompts immediately,
          - Magnetizes newly gained Magnetic Mechs directly onto Mech allies even on a 7-minion board!,
          - Passes `board_units`, `obs_trinkets`, and `lobby_tribes` into `effective_engine_bonus`.
        """
        self._fast_resolve_chained_discover(sim, particles=particles, ctx=ctx)
        obs_after = sim.observe()
        if obs_after.discover_pending is not None or obs_after.trinket_options or not obs_after.hand:
            return
        new_idx = len(obs_after.hand) - 1
        new_card = obs_after.hand[new_idx]
        if new_card.unplayable:
            return

        lobby_tribes = ctx.tribes if ctx is not None else None

        if new_card.is_spell:
            if new_card.card_id in (self.kb.robust_evolution_id, 847, 848, 863):
                return
            fast_cast = seaglass.TavernAction.play(new_idx, 0)
            if sim.is_legal(fast_cast):
                sim.step(fast_cast)
                self._fast_resolve_chained_discover(sim, particles=particles, ctx=ctx)
                return
            for sub in sim.legal_actions():
                if sub.kind == "play" and sub.args[0] == new_idx:
                    sim.step(sub)
                    self._fast_resolve_chained_discover(sim, particles=particles, ctx=ctx)
                    break
            return

        # Direct Magnetize onto best Mech/All ally on board (works on 7-minion boards without selling!)
        if new_card.magnetic and obs_after.board:
            mech_targets = [
                (
                    (12.0 if u.divine_shield and not new_card.divine_shield else 0.0)
                    + (10.0 if not u.divine_shield and new_card.divine_shield else 0.0)
                    + u.attack
                    + u.health,
                    b_i,
                )
                for b_i, u in enumerate(obs_after.board)
                if "mech" in u.tribes or "all" in u.tribes
            ]
            if mech_targets and (len(obs_after.board) >= 7 or new_card.divine_shield):
                best_m_idx = max(mech_targets, key=lambda x: x[0])[1]
                mag_act = seaglass.TavernAction.play(new_idx, best_m_idx)
                if sim.is_legal(mag_act):
                    sim.step(mag_act)
                    return

        tribe_counts: Dict[str, int] = {}
        for u in obs_after.board:
            for tr in u.tribes:
                tribe_counts[tr] = tribe_counts.get(tr, 0) + 1

        if len(obs_after.board) < 7:
            best_pos = len(obs_after.board)
            if new_card.card_id == self.kb.faceless_manipulator_id and obs_after.board:
                copy_cands = [
                    (
                        (18.0 if u.is_golden else 0.0)
                        + self.evaluator.effective_engine_bonus(
                            u.card_id,
                            obs_after.turn,
                            tribe_counts,
                            on_board=True,
                            board_units=obs_after.board,
                            is_golden=u.is_golden,
                            obs_trinkets=obs_after.trinkets,
                            lobby_tribes=lobby_tribes,
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
                            u.card_id,
                            obs_after.turn,
                            tribe_counts,
                            on_board=True,
                            board_units=obs_after.board,
                            obs_trinkets=obs_after.trinkets,
                            lobby_tribes=lobby_tribes,
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
                n_hand_before = len(obs_after.hand)
                sim.step(play_act)
                self._drain_post_play_rewards(sim, n_hand_before - 1, particles=particles, ctx=ctx)
        elif len(obs_after.board) == 7 and not self.evaluator.has_hand_buff_synergy(obs_after):
            weak = expendable_slots(obs_after, self.evaluator, max_slots=1, particles=particles, ctx=ctx)
            if weak:
                w_slot = weak[0]
                w_unit = obs_after.board[w_slot]
                new_eng = self.evaluator.effective_engine_bonus(
                    new_card.card_id,
                    obs_after.turn,
                    tribe_counts,
                    on_board=False,
                    board_units=obs_after.board,
                    is_golden=new_card.is_golden,
                    obs_trinkets=obs_after.trinkets,
                    lobby_tribes=lobby_tribes,
                )
                w_eng = self.evaluator.effective_engine_bonus(
                    w_unit.card_id,
                    obs_after.turn,
                    tribe_counts,
                    on_board=True,
                    board_units=obs_after.board,
                    is_golden=w_unit.is_golden,
                    obs_trinkets=obs_after.trinkets,
                    lobby_tribes=lobby_tribes,
                )
                w_scam = w_slot in _protected_scam_slots(obs_after, self.kb, particles)
                new_is_scam = new_card.venomous or new_card.card_id in (
                    self.kb.leeroy_id,
                    self.kb.morgl_id,
                    self.kb.sindorei_id,
                )
                new_prof = self.meta.profiles.get(new_card.card_id)
                is_cycle_slot = (
                    (w_unit.attack + w_unit.health) < _cycle_stat_cap(obs_after.turn)
                    and w_eng < 10.0
                ) or (
                    w_unit.card_id in self.kb.true_on_sell_plain_ids
                    or (w_unit.card_id in self.kb.golden_only_sell_ids and w_unit.is_golden)
                )
                is_cycle_play = is_cycle_slot and (
                    (new_prof is not None and new_prof.has_battlecry)
                    or new_card.card_id in self.kb.true_on_sell_plain_ids
                )
                if not w_scam and (
                    new_card.is_golden
                    or new_card.card_id == self.kb.faceless_manipulator_id
                    or new_eng > w_eng + 3.0
                    or is_cycle_play
                    or (new_is_scam and (w_unit.attack + w_unit.health) < 260 and w_eng < 11.0)
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
                                n_hand_mid = len(obs_mid.hand)
                                sim.step(p_act)
                                self._drain_post_play_rewards(
                                    sim, n_hand_mid - 1, particles=particles, ctx=ctx
                                )

    def _drain_post_play_rewards(
        self,
        sim: seaglass.TavernGame,
        expected_hand_len: int,
        particles: Optional[SovereignOpponentParticles] = None,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> None:
        """
        After playing a minion in rollout (e.g. a Golden Triple or Battlecry Discover/Spell generator):
          1. Resolve any immediate `discover_pending` and play the discovered card,
          2. Cast any newly generated 0-cost spell (such as a Triple Reward Discover spell!),
             resolve the resulting Triple Discover, and play the discovered Tier t+1 carry!
        """
        for _ in range(2):
            obs = sim.observe()
            if obs.discover_pending is not None:
                self._fast_resolve_chained_discover(sim, particles=particles, ctx=ctx)
                obs = sim.observe()
                if len(obs.hand) > expected_hand_len:
                    self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
                continue
            if len(obs.hand) > expected_hand_len:
                last_idx = len(obs.hand) - 1
                last_c = obs.hand[last_idx]
                if (
                    last_c.is_spell
                    and last_c.spell_cost == 0
                    and not last_c.unplayable
                    and last_c.card_id not in (self.kb.robust_evolution_id, 847, 848, 863)
                ):
                    cast0 = seaglass.TavernAction.play(last_idx, 0)
                    stepped = False
                    if sim.is_legal(cast0):
                        sim.step(cast0)
                        stepped = True
                    else:
                        for la in sim.legal_actions():
                            if la.kind == "play" and la.args[0] == last_idx:
                                sim.step(la)
                                stepped = True
                                break
                    if stepped:
                        if sim.observe().discover_pending is not None:
                            self._fast_resolve_chained_discover(sim, particles=particles, ctx=ctx)
                            self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
                        continue
            break

    def _evaluate_discover_options(
        self,
        base_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        particles: SovereignOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
        allow_shop_continuation: bool = True,
    ) -> seaglass.TavernAction:
        """
        CRN-paired + Full-Turn Continuation Discover Evaluator:
          Simulates playing the discovered card AND spending remaining gold in the current shop
          so mid-turn economy/engine Discovers (`Brann`, `Recycling Wraith`, `Hoggarr`) realize their true EV!
        """
        best_act = legal[0]
        best_val = -1e18
        crn_seed = self.rng.getrandbits(63) | 1

        for act in legal:
            sim = base_game.copy()
            sim.reseed(crn_seed)
            sim.step(act)
            self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
            obs_post = sim.observe()
            if allow_shop_continuation and (
                obs_post.gold >= 2
                or any(not c.unplayable and (c.is_spell or len(obs_post.board) < 7) for c in obs_post.hand)
            ):
                sim = self._rollout_shop_consumer(
                    sim,
                    ctx,
                    particles=particles,
                    max_steps=3,
                    crn_seed=(crn_seed ^ 0x27D4EB2F165667C5) | 1,
                )
            val = self.evaluator.evaluate_turn_end(
                sim, particles, ctx=ctx, combats_per_particle=10
            )
            if val > best_val:
                best_val = val
                best_act = act
        return best_act

    def _choose_best_trinket(
        self,
        base_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        particles: SovereignOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
    ) -> seaglass.TavernAction:
        """
        CRN-Paired (`M = 2` seeds) 2-Turn Exact Rust Physics Rollout for Turn 6 & Turn 9 Trinkets!
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

        # CRN seed pair shared identically across all offered Trinkets!
        crn_pair = [self.rng.getrandbits(63) | 1, self.rng.getrandbits(63) | 1]

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
                    t_prior += 0.75 * self.sov_kb.trinket_prior_score(tname)
                    if tname == "Weighted Gauntlet":
                        t_prior += 35.0
                    elif tname in (
                        "Souvenir Stand",
                        "Worn Treasure Map",
                        "Stuffed Coin Purse",
                        "Trip Vouchers",
                        "Goblin Wallet",
                        "Transcribing Typewriter",
                        "Bartend-o-Tron's Oilcan",
                        "Book of Medivh",
                    ):
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

            rollout_sum = 0.0
            for m_idx, r_seed in enumerate(crn_pair):
                sim = base_game.copy()
                sim.reseed(r_seed)
                sim.step(act)

                for _ in range(3):
                    s_obs = sim.observe()
                    if s_obs.discover_pending is not None:
                        d_legal = sim.legal_actions()
                        if d_legal:
                            pick = self._evaluate_discover_options(
                                sim, d_legal, particles, ctx, allow_shop_continuation=False
                            )
                            sim.step(pick)
                            self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
                    elif s_obs.trinket_options:
                        t_legal = sim.legal_actions()
                        if t_legal:
                            sim.step(t_legal[0])
                    else:
                        break

                self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
                if tid == self.kb.guiding_candle_id and sim.observe().gold >= 4:
                    ref0 = seaglass.TavernAction.refresh()
                    if sim.is_legal(ref0):
                        sim.step(ref0)

                sim_t0 = self._rollout_shop_consumer(
                    sim, ctx, particles=particles, max_steps=4, crn_seed=(r_seed ^ 0x13579BDF) | 1
                )
                v_turn0 = self.evaluator.evaluate_turn_end(
                    sim_t0, particles, ctx=ctx, combats_per_particle=8
                )

                v_turn1_macro = 0.0
                if particles.boards:
                    sim_t1 = sim_t0.copy()
                    self.positioner.fast_reposition_game(sim_t1, sim_t1.observe(), particles)
                    c_seed = (obs0.turn * 262147 + m_idx * 4099 + 5) | 1
                    sim_t1.reseed(c_seed)
                    if sim_t1.is_legal(seaglass.TavernAction.end_turn()):
                        sim_t1.end_turn()
                    opp_b = particles.boards[m_idx % len(particles.boards)]
                    sim_t1.resolve_combat_against(
                        opp_b, opponent_tier=particles.tier, seed=c_seed
                    )
                    sim_t1.start_turn()
                    # Fix ApexBot Bug #12: properly evaluate & PLAY Start-of-Turn Trinket Discovers on Turn t+1!
                    for _ in range(2):
                        if sim_t1.observe().discover_pending is not None:
                            self._fast_resolve_chained_discover(sim_t1, particles=particles, ctx=ctx)
                            self._play_newly_gained_card_in_sim(sim_t1, particles=particles, ctx=ctx)
                        else:
                            break
                    if tid == self.kb.guiding_candle_id:
                        ref1 = seaglass.TavernAction.refresh()
                        if sim_t1.is_legal(ref1):
                            sim_t1.step(ref1)
                    sim_t1 = self._rollout_shop_consumer(
                        sim_t1, ctx, particles=particles, max_steps=4, crn_seed=(c_seed ^ 0x2468ACE1) | 1
                    )
                    self.positioner.fast_reposition_game(sim_t1, sim_t1.observe(), particles)
                    if sim_t1.is_legal(seaglass.TavernAction.end_turn()):
                        sim_t1.end_turn()
                    sim_t1.resolve_combat_against(
                        opp_b, opponent_tier=particles.tier, seed=c_seed + 2
                    )
                    sim_t1.start_turn()
                    v_turn1_macro = self.evaluator.evaluate_fast(
                        sim_t1.observe(),
                        ctx=ctx,
                        end_of_turn=False,
                        particles=particles,
                        in_dist_sot=True,
                    )

                rollout_sum += 0.68 * v_turn0 + 0.32 * v_turn1_macro

            total_val = (rollout_sum / len(crn_pair)) + t_prior
            if total_val > best_val:
                best_val = total_val
                best_act = act

        return best_act

    def _expand_cycle_macros(
        self,
        g: seaglass.TavernGame,
        obs: seaglass.TavernObservation,
        path: List[QueuedStep],
        particles: Optional[SovereignOpponentParticles] = None,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]]:
        if len(obs.board) < 7 or not obs.hand:
            return []
        weak_slots = expendable_slots(obs, self.evaluator, max_slots=2, particles=particles, ctx=ctx)
        if not weak_slots:
            return []

        lobby_tribes = ctx.tribes if ctx is not None else None
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
        cycle_cap = _cycle_stat_cap(obs.turn)
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
                lobby_tribes=lobby_tribes,
            )
            is_true_sell = (
                w_unit.card_id in self.kb.true_on_sell_plain_ids
                or (w_unit.card_id in self.kb.golden_only_sell_ids and w_unit.is_golden)
            )
            w_is_scam = w_slot in _protected_scam_slots(obs, self.kb, particles)
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
                lobby_tribes=lobby_tribes,
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
                    lobby_tribes=lobby_tribes,
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
                    lobby_tribes=lobby_tribes,
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

    @staticmethod
    def _select_stratified_roll_prefixes(
        roll_prefixes: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]],
        root_fast: float,
        root_obs: seaglass.TavernObservation,
        max_prefixes: int = 4,
    ) -> List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]]:
        """
        Cures ApexBot's Roll-Prefix Crowding Bug:
          1. Ranks prefixes by `p_fast + roll_continuation_bonus(remaining_gold, free_refreshes)`
             so `Buy(1 S-tier card) + 7g left` isn't evicted by `Buy(S-tier) + Buy(filler) + 4g left`!
          2. Stratifies across:
             - Best high-gold short prefix (`len(path) <= 2`, e.g. single buy+play or upgrade),
             - Best surplus-per-gold-spent prefix,
             - Top remaining continuation-ranked prefixes (up to `max_prefixes` unique states).
        """
        valid: List[Tuple[float, float, float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep], bool]] = []
        seen_keys: set[Tuple] = set()
        min_keep_board = min(len(root_obs.board), 7 if root_obs.turn >= 7 else 6)
        for p_fast, p_g, p_obs, p_path in roll_prefixes:
            has_clean_upgrade = (
                p_obs.tavern_tier > root_obs.tavern_tier
                and len(p_obs.board) >= min_keep_board
                and (SovereignEvaluator.effective_hp(p_obs) >= 20.0 or root_obs.upgrade_cost <= 3)
            )
            # Clean upgrades convert current gold into higher tier for `Upgrade -> Refresh -> Buy`,
            # whereas board-cannibalizing `[Upgrade, Sell, Sell, Refresh]` paths get rejected!
            min_fast_floor = (root_fast - 6.0) if has_clean_upgrade else (root_fast - 0.8)
            if p_fast < min_fast_floor:
                continue
            pk = _canonical_state_key(p_obs)
            if pk in seen_keys:
                continue
            seen_keys.add(pk)
            rem_rolls = max(0, p_obs.gold - p_obs.refresh_cost)
            cont_score = (
                p_fast
                + (0.46 + 0.06 * max(0, p_obs.tavern_tier - 3)) * rem_rolls
                + 0.60 * p_obs.free_refreshes
            )
            gold_spent = max(1, (root_obs.gold - p_obs.gold) + (root_obs.free_refreshes - p_obs.free_refreshes))
            eff_ratio = (p_fast - root_fast) / float(gold_spent)
            valid.append((cont_score, eff_ratio, p_fast, p_g, p_obs, p_path, has_clean_upgrade))

        if not valid:
            return []

        selected: List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []
        used_idx: set[int] = set()

        # Dedicated Upgrade-Continuation Slot: always evaluate `Upgrade -> Refresh` when affordable!
        upg_cands = [i for i, it in enumerate(valid) if it[6]]
        eff_max_prefixes = (max_prefixes + 1) if upg_cands else max_prefixes
        if upg_cands:
            b_upg = max(upg_cands, key=lambda i: valid[i][0])
            used_idx.add(b_upg)
            it = valid[b_upg]
            selected.append((it[2], it[3], it[4], it[5]))

        # Slot 1: Best short / high-remaining-gold prefix (`len(path) <= 2`)
        short_cands = [i for i, it in enumerate(valid) if len(it[5]) <= 2 and i not in used_idx]
        if short_cands and len(selected) < eff_max_prefixes:
            b_short = max(short_cands, key=lambda i: valid[i][0])
            used_idx.add(b_short)
            it = valid[b_short]
            selected.append((it[2], it[3], it[4], it[5]))

        # Slot 2: Best efficiency-per-gold-spent prefix
        rem_cands = [i for i in range(len(valid)) if i not in used_idx]
        if rem_cands and len(selected) < eff_max_prefixes:
            b_eff = max(rem_cands, key=lambda i: valid[i][1])
            used_idx.add(b_eff)
            it = valid[b_eff]
            selected.append((it[2], it[3], it[4], it[5]))

        # Remaining slots: highest `cont_score`
        by_cont = sorted(range(len(valid)), key=lambda i: valid[i][0], reverse=True)
        for i in by_cont:
            if len(selected) >= eff_max_prefixes:
                break
            if i in used_idx:
                continue
            used_idx.add(i)
            it = valid[i]
            selected.append((it[2], it[3], it[4], it[5]))

        return selected

    def _search_shop_epoch(
        self,
        root_game: seaglass.TavernGame,
        particles: SovereignOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
    ) -> Tuple[float, List[QueuedStep], List[Tuple[float, seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]]]:
        trace = self._tracing()
        n_layers = 0
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
        eff_depth = min(16, self.max_epoch_depth + zero_cost_spells)
        for _depth in range(eff_depth):
            n_layers += 1
            raw_children: List[Tuple[seaglass.TavernGame, seaglass.TavernObservation, List[QueuedStep]]] = []

            for b_fast, g, obs, path in beam:
                obs_sig = _plan_validation_key(obs)
                end_step: QueuedStep = (("end_turn", None, None), obs_sig, False)
                raw_terminals.append((g, obs, True, path + [end_step]))

                if path and obs.discover_pending is None and not obs.trinket_options and self._can_productive_refresh(obs):
                    roll_prefixes.append((b_fast, g, obs, path))

                # Allow freezing when `< minion_cost` OR when `gold <= 3` on a full 7-board!
                if (
                    not obs.is_frozen
                    and obs.shop
                    and (obs.gold < obs.minion_cost or (obs.gold <= 3 and len(obs.board) >= 7))
                ):
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

                legal = prune_sovereign_actions(
                    obs, g.legal_actions(), self.evaluator, self.kb, particles=particles, ctx=ctx
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

                raw_children.extend(
                    self._expand_cycle_macros(g, obs, path, particles=particles, ctx=ctx)
                )

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

        eot_indices = [i for i, item in enumerate(raw_terminals) if item[2]]
        open_indices = [i for i, item in enumerate(raw_terminals) if not item[2]]
        term_scores = [0.0] * len(raw_terminals)
        term_parts: List[Dict[str, float]] = [{} for _ in raw_terminals]

        if eot_indices:
            eot_parts: Optional[List[Dict[str, float]]] = [] if trace is not None else None
            vals = self.evaluator.evaluate_fast_batch(
                [raw_terminals[i][1] for i in eot_indices],
                ctx=ctx,
                end_of_turn=True,
                particles=particles,
                **({"parts": eot_parts} if trace is not None else {}),
            )
            for k, (idx, v) in enumerate(zip(eot_indices, vals)):
                term_scores[idx] = v
                if eot_parts:
                    term_parts[idx] = eot_parts[k]
        if open_indices:
            open_parts: Optional[List[Dict[str, float]]] = [] if trace is not None else None
            vals = self.evaluator.evaluate_fast_batch(
                [raw_terminals[i][1] for i in open_indices],
                ctx=ctx,
                end_of_turn=False,
                particles=particles,
                **({"parts": open_parts} if trace is not None else {}),
            )
            for k, (idx, v) in enumerate(zip(open_indices, vals)):
                term_scores[idx] = v
                if open_parts:
                    term_parts[idx] = open_parts[k]

        ranked_terms: List[
            Tuple[float, seaglass.TavernGame, List[QueuedStep], Dict[str, float], seaglass.TavernObservation]
        ] = []
        seen_term_keys: set[Tuple] = set()
        for t_score, t_parts, (t_game, t_obs, t_eot, t_path) in sorted(
            zip(term_scores, term_parts, raw_terminals), key=lambda x: x[0], reverse=True
        ):
            t_sig = (_canonical_state_key(t_obs), t_eot)
            if t_sig in seen_term_keys:
                continue
            seen_term_keys.add(t_sig)
            ranked_terms.append((t_score, t_game, t_path, t_parts, t_obs))
            if len(ranked_terms) >= self.top_k_combat_evals:
                break

        best_score = -1e18
        default_end: QueuedStep = (
            ("end_turn", None, None),
            _plan_validation_key(root_obs),
            False,
        )
        best_path: List[QueuedStep] = [default_end]

        for cand_fast, cand_game, cand_path, cand_fast_parts, cand_term_obs in ranked_terms:
            eval_game = cand_game
            cand_obs = cand_game.observe()
            if cand_obs.discover_pending is not None:
                eval_game = cand_game.copy()
                eval_game.reseed(self.rng.getrandbits(63) | 1)
                d_legal = eval_game.legal_actions()
                if d_legal:
                    d_pick = self._evaluate_discover_options(
                        eval_game, d_legal, particles, ctx, allow_shop_continuation=False
                    )
                    eval_game.step(d_pick)
                    self._play_newly_gained_card_in_sim(eval_game, particles=particles, ctx=ctx)
                eval_game = self._rollout_shop_consumer(eval_game, ctx, particles=particles, max_steps=3)
            elif cand_path and cand_path[-1][2]:
                eval_game = self._rollout_shop_consumer(cand_game, ctx, particles=particles, max_steps=3)

            line_parts: Optional[Dict[str, float]] = {} if trace is not None else None
            full_score = self.evaluator.evaluate_turn_end(
                eval_game,
                particles,
                ctx=ctx,
                combats_per_particle=14,
                **({"parts": line_parts} if trace is not None else {}),
            )
            if trace is not None:
                trace.plans.append(
                    PlanTrace(
                        label="plan",
                        actions=self._step_keys(cand_path),
                        score=float(full_score),
                        fast=float(cand_fast),
                        fast_parts=dict(cand_fast_parts),
                        parts=dict(line_parts or {}),
                        terminal_obs=cand_term_obs,
                    )
                )
            if full_score > best_score:
                best_score = full_score
                best_path = cand_path

        top_prefixes = self._select_stratified_roll_prefixes(
            roll_prefixes, root_fast=root_fast, root_obs=root_obs, max_prefixes=4
        )
        if trace is not None:
            trace.counts.update(
                terminals=len(raw_terminals),
                beam_layers=n_layers,
                lines_scored=len(ranked_terms),
                roll_prefixes=len(roll_prefixes),
                roll_prefixes_selected=len(top_prefixes),
            )
        return best_score, best_path, top_prefixes

    def _rollout_shop_consumer(
        self,
        start_game: seaglass.TavernGame,
        ctx: Optional[seaglass.LobbyContext],
        particles: Optional[SovereignOpponentParticles] = None,
        max_steps: int = 5,
        crn_seed: Optional[int] = None,
    ) -> seaglass.TavernGame:
        """
        Fixed & Upgraded Multi-Step Shop Consumer after Refresh / Discover:
          - Cures ApexBot Bug #11 (inverted `is_spell` sort that left bought minions unplayed in hand!),
          - Plays bought minions, casts Triple Rewards & resolves Triple Discovers,
          - Rolls up to `3` additional times past empty/trash shops when `gold >= 4` without ending at dead gold!
        """
        sim = start_game.copy()
        base_seed = crn_seed if crn_seed is not None else (self.rng.getrandbits(63) | 1)
        sim.reseed(base_seed)
        rolls_used = 0
        actions_taken = 0

        for _iter in range(max_steps + 3):
            if actions_taken >= max_steps:
                break
            obs = sim.observe()
            if obs.discover_pending is not None:
                self._fast_resolve_chained_discover(sim, particles=particles, ctx=ctx)
                self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
                continue

            cur_eot = self.evaluator.evaluate_fast(
                obs, ctx=ctx, end_of_turn=True, particles=particles
            )
            cur_total = cur_eot + 1.38 * obs.gold + 0.88 * obs.free_refreshes
            cand_sims: List[seaglass.TavernGame] = []
            cand_obs: List[seaglass.TavernObservation] = []

            pruned = prune_sovereign_actions(
                obs, sim.legal_actions(), self.evaluator, self.kb, particles=particles, ctx=ctx
            )
            for act in pruned:
                if act.kind in ("end_turn", "refresh", "toggle_freeze", "pass", "dark_discovery"):
                    continue
                if act.kind in ("hero_power", "hero_power_2") and self._is_stochastic_hero_power(sim, obs, act):
                    continue
                c = sim.copy()
                c.reseed((base_seed + _iter * 1009 + 17) | 1)
                c.step(act)
                if act.kind == "buy":
                    c_play = c.copy()
                    self._play_newly_gained_card_in_sim(c_play, particles=particles, ctx=ctx)
                    cp_obs = c_play.observe()
                    cand_sims.append(c_play)
                    cand_obs.append(cp_obs)
                    if len(obs.board) >= 7:
                        c_obs = c.observe()
                        if len(cp_obs.hand) != len(c_obs.hand):
                            cand_sims.append(c)
                            cand_obs.append(c_obs)
                else:
                    cand_sims.append(c)
                    cand_obs.append(c.observe())

            for c_game, c_o, _ in self._expand_cycle_macros(
                sim, obs, [], particles=particles, ctx=ctx
            ):
                cand_sims.append(c_game)
                cand_obs.append(c_o)

            can_roll_more = (
                rolls_used < 3
                and self._can_productive_refresh(obs)
                and (obs.gold >= 4 or (obs.free_refreshes > 0 and obs.gold >= 2))
            )

            if not cand_sims:
                if can_roll_more:
                    ref = seaglass.TavernAction.refresh()
                    if sim.is_legal(ref):
                        sim.step(ref)
                        rolls_used += 1
                        continue
                break

            eot_scores = self.evaluator.evaluate_fast_batch(
                cand_obs, ctx=ctx, end_of_turn=True, particles=particles
            )
            scores = [
                ev + 1.38 * co.gold + 0.88 * co.free_refreshes
                for ev, co in zip(eot_scores, cand_obs)
            ]
            best_idx = max(range(len(scores)), key=lambda i: scores[i])
            if scores[best_idx] > cur_total - 0.12:
                sim = cand_sims[best_idx]
                actions_taken += 1
            elif can_roll_more:
                ref = seaglass.TavernAction.refresh()
                if sim.is_legal(ref):
                    sim.step(ref)
                    rolls_used += 1
                    continue
                break
            else:
                break
        return sim

    def _evaluate_stochastic_action(
        self,
        base_game: seaglass.TavernGame,
        stoch_act: seaglass.TavernAction,
        particles: SovereignOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
        crn_seeds: Optional[Sequence[int]] = None,
        n_samples: Optional[int] = None,
        parts: Optional[Dict[str, float]] = None,
    ) -> float:
        """Mean `evaluate_turn_end` over CRN samples; `parts` (optional out-dict) gets the mean breakdown."""
        samples = n_samples if n_samples is not None else self.refresh_samples
        if crn_seeds is not None and len(crn_seeds) >= samples:
            seeds = crn_seeds[:samples]
        else:
            seeds = [self.rng.getrandbits(63) | 1 for _ in range(samples)]

        total = 0.0
        parts_sum: Dict[str, float] = {}
        for s_val in seeds:
            sim = base_game.copy()
            sim.reseed(s_val)
            sim.step(stoch_act)
            legal_after = sim.legal_actions()
            if legal_after and legal_after[0].kind == "choose_discover":
                pick = self._evaluate_discover_options(
                    sim, legal_after, particles, ctx, allow_shop_continuation=False
                )
                sim.step(pick)
                self._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)

            sim = self._rollout_shop_consumer(
                sim,
                ctx,
                particles=particles,
                max_steps=4,
                crn_seed=(s_val ^ 0x9E3779B97F4A7C15) | 1,
            )
            if parts is not None:
                one: Dict[str, float] = {}
                total += self.evaluator.evaluate_turn_end(
                    sim, particles, ctx=ctx, combats_per_particle=8, parts=one
                )
                for k, v in one.items():
                    parts_sum[k] = parts_sum.get(k, 0.0) + v
            else:
                total += self.evaluator.evaluate_turn_end(
                    sim, particles, ctx=ctx, combats_per_particle=8
                )
        if parts is not None:
            parts.update({k: v / max(1, len(seeds)) for k, v in parts_sum.items()})
        return total / max(1, len(seeds))

    def _begin_trace(self, player: int, turn: int) -> DecisionTrace:
        self.last_note = None
        trace = DecisionTrace(player=int(player), turn=int(turn))
        self.last_trace = trace
        self._trace = trace
        return trace

    def _end_trace(self, trace: DecisionTrace, act: seaglass.TavernAction) -> None:
        trace.chosen = _action_key(act)
        self.last_note = note_from_trace(trace, self.last_note)
        self._trace = None

    def _set_queue(
        self, player: int, steps: List[QueuedStep], plan_value: Optional[float], label: str
    ) -> None:
        self._queued_actions[player] = steps
        self._queue_meta[player] = (plan_value, label)

    def _mark_queued(self, trace: Optional[DecisionTrace], player: int, steps_left: int) -> None:
        if trace is None:
            return
        trace.kind = "queued"
        pv, label = self._queue_meta.get(player, (None, "plan"))
        trace.queued = {"plan_value": pv, "steps_left": int(steps_left), "label": label}

    def choose_action_for_tavern(
        self,
        belief_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        player: int = 0,
        ctx: Optional[seaglass.LobbyContext] = None,
        lobby_id: int = 0,
    ) -> seaglass.TavernAction:
        own_trace = self._trace is None
        if own_trace:
            self._begin_trace(player, belief_game.observe().turn)
        try:
            act = self._choose_action_impl(belief_game, legal, player, ctx, lobby_id)
        except BaseException:
            if own_trace:
                self._trace = None
            raise
        if own_trace and self._trace is not None:
            self._end_trace(self._trace, act)
        return act

    def _choose_action_impl(
        self,
        belief_game: seaglass.TavernGame,
        legal: Sequence[seaglass.TavernAction],
        player: int,
        ctx: Optional[seaglass.LobbyContext],
        lobby_id: int,
    ) -> seaglass.TavernAction:
        trace = self._trace
        heavy = self._tracing() is not None  # plan scores / breakdowns need an evaluator that supports `parts`
        obs = belief_game.observe()
        self.last_note = None
        if not legal:
            if trace is not None:
                trace.kind = "only_legal"
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            if trace is not None:
                trace.kind = "only_legal"
            return legal[0]

        if legal[0].kind == "choose_hero":
            if trace is not None:
                trace.kind = "hero"
            return self.evaluator.choose_best_hero(obs, legal, ctx=ctx, base_game=belief_game)

        particles = self.belief.next_opponent_particles(
            obs.turn, player, ctx, lobby_id=lobby_id
        )
        if trace is not None:
            trace.game = belief_game
            trace.particles = particles
            trace.ctx = ctx
            trace.legal = list(legal)

        if legal[0].kind == "choose_trinket":
            self._queued_actions.pop(player, None)
            if trace is not None:
                trace.kind = "trinket"
            best_t = self._choose_best_trinket(belief_game, legal, particles, ctx)
            self.last_note = {"trinket_crn_rollout": 2.0}
            return best_t

        if legal[0].kind == "choose_discover":
            self._queued_actions.pop(player, None)
            if trace is not None:
                trace.kind = "discover"
            best_d = self._evaluate_discover_options(belief_game, legal, particles, ctx)
            self.last_note = {"discover_continuation": 1.0}
            return best_d

        queued = self._queued_actions.get(player)
        if queued:
            next_key, exp_sig, stop_after = queued.pop(0)
            if _plan_validation_key(obs) == exp_sig:
                if next_key[0] == "end_turn":
                    self._queued_actions.pop(player, None)
                    self._mark_queued(trace, player, 0)
                    return self._finalize_positioning_or_end(
                        belief_game, legal, player, obs.turn, particles, ctx, lobby_id=lobby_id
                    )
                matched = self._find_matching_legal(legal, next_key)
                if matched is not None:
                    self._mark_queued(trace, player, len(queued))
                    if stop_after or not queued:
                        self._queued_actions.pop(player, None)
                    self.last_note = {"queued_plan": float(len(queued) + 1)}
                    return matched
            self._queued_actions.pop(player, None)

        # 1. Search current shop epoch (both `EndTurn` paths AND stratified `Buy/Upgrade -> Refresh` prefixes!)
        epoch_score, epoch_path, top_roll_prefixes = self._search_shop_epoch(
            belief_game, particles, ctx
        )

        best_plan_score = epoch_score
        best_plan_path: List[QueuedStep] = epoch_path
        best_plan_label = "plan"
        best_immediate_act: Optional[seaglass.TavernAction] = None
        best_pt: Optional[PlanTrace] = None
        if heavy:
            epoch_keys = self._step_keys(epoch_path)
            for pt in trace.plans:
                if pt.label == "plan" and pt.actions == epoch_keys:
                    best_pt = pt
                    break
        margin = 0.22

        def _scored(
            label: str, keys: List[ActionKey], score: float, parts: Dict[str, float], samples: int
        ) -> Optional[PlanTrace]:
            if trace is None or not heavy:
                return None
            pt = PlanTrace(
                label=label, actions=keys, score=float(score), parts=dict(parts), samples=int(samples)
            )
            trace.plans.append(pt)
            return pt

        def _stoch(act: seaglass.TavernAction, g: seaglass.TavernGame, n: Optional[int] = None):
            parts: Optional[Dict[str, float]] = {} if heavy else None
            sc = self._evaluate_stochastic_action(
                g, act, particles, ctx, crn_seeds=crn_seeds, n_samples=n,
                **({"parts": parts} if heavy else {}),
            )
            return sc, (parts or {}), (n if n is not None else self.refresh_samples)

        # Shared Common Random Numbers stream across all competing stochastic actions this turn!
        crn_seeds = tuple(self.rng.getrandbits(63) | 1 for _ in range(self.refresh_samples))

        # 2. Evaluate `DarkDiscovery`
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
        dd_legal = belief_game.is_legal(dd_act)
        if trace is not None and dd_legal:
            if allow_dd:
                dd_detail = "Dark Discovery allowed (tier %d, turn %d, effective HP %.0f)" % (
                    obs.tavern_tier, obs.turn, ehp_now)
            else:
                dd_detail = (
                    "Dark Discovery not scored: needs tier >= 3, or tier 2 from turn 4, or <= 2 players alive "
                    "from turn 3, or no damage cap, or effective HP <= 20 (tier %d, turn %d, effective HP %.0f, "
                    "damage cap %s)" % (obs.tavern_tier, obs.turn, ehp_now, dmg_cap)
                )
            trace.gates.append(
                GateTrace(
                    "dark_discovery",
                    bool(allow_dd),
                    dd_detail,
                    {
                        "tavern_tier": obs.tavern_tier,
                        "turn": obs.turn,
                        "ehp": float(ehp_now),
                        "alive_units": alive_units,
                        "damage_cap": dmg_cap,
                    },
                )
            )
        if allow_dd and dd_legal:
            dd_score, dd_parts, dd_n = _stoch(dd_act, belief_game)
            dd_pt = _scored("dark discovery", [_action_key(dd_act)], dd_score, dd_parts, dd_n)
            if dd_score > best_plan_score + margin:
                best_plan_score = dd_score
                best_immediate_act = dd_act
                best_pt = dd_pt

        # 3. Evaluate stochastic / shop-mutating `HeroPower` & `HeroPower2`
        for act in legal:
            if act.kind in ("hero_power", "hero_power_2") and self._is_stochastic_hero_power(
                belief_game, obs, act
            ):
                hp_score, hp_parts, hp_n = _stoch(act, belief_game)
                hp_pt = _scored("hero power", [_action_key(act)], hp_score, hp_parts, hp_n)
                if hp_score > best_plan_score + margin:
                    best_plan_score = hp_score
                    best_immediate_act = act
                    best_pt = hp_pt

        # 4. Evaluate root `Refresh`
        ref_act = seaglass.TavernAction.refresh()
        ref_legal = belief_game.is_legal(ref_act)
        can_prod = self._can_productive_refresh(obs)
        if trace is not None:
            if obs.free_refreshes > 0:
                need = 1 if obs.minion_cost != 0 else 0
                need_txt = "a free refresh needs gold >= 1 (or a free minion)"
            else:
                need = obs.refresh_cost + min(2, max(1, obs.minion_cost))
                need_txt = "roll %d + %d to buy afterwards" % (obs.refresh_cost, min(2, max(1, obs.minion_cost)))
            if not ref_legal:
                rr_detail = "Refresh is not legal (gold %d, cost %d)" % (obs.gold, obs.refresh_cost)
            elif can_prod:
                rr_detail = "gold %d >= %d needed (%s): scored" % (obs.gold, need, need_txt)
            else:
                rr_detail = "gold %d < %d needed (%s): roll not scored" % (obs.gold, need, need_txt)
            trace.gates.append(
                GateTrace(
                    "root_refresh",
                    bool(ref_legal and can_prod),
                    rr_detail,
                    {
                        "legal": bool(ref_legal),
                        "productive": bool(can_prod),
                        "gold": obs.gold,
                        "refresh_cost": obs.refresh_cost,
                        "minion_cost": obs.minion_cost,
                        "free_refreshes": obs.free_refreshes,
                        "gold_needed": need,
                    },
                )
            )
        if ref_legal and can_prod:
            ref_score, ref_parts, ref_n = _stoch(ref_act, belief_game)
            ref_pt = _scored("roll", [_action_key(ref_act)], ref_score, ref_parts, ref_n)
            if ref_score > best_plan_score + margin:
                best_plan_score = ref_score
                best_immediate_act = ref_act
                best_pt = ref_pt

        # 5. Evaluate stratified `Buy/Play/Upgrade -> Refresh` continuation prefixes (CRN-paired)!
        pref_samples = max(4, self.refresh_samples - 1)
        n_pref_scored = 0
        for _p_fast, pref_game, pref_obs, pref_path in top_roll_prefixes:
            pref_ref = seaglass.TavernAction.refresh()
            if pref_game.is_legal(pref_ref):
                pref_roll_score, pref_parts, pref_n = _stoch(pref_ref, pref_game, pref_samples)
                n_pref_scored += 1
                pref_pt = _scored(
                    "then roll",
                    self._step_keys(pref_path) + [_action_key(pref_ref)],
                    pref_roll_score,
                    pref_parts,
                    pref_n,
                )
                if pref_roll_score > best_plan_score + margin:
                    best_plan_score = pref_roll_score
                    best_immediate_act = None
                    ref_step: QueuedStep = (
                        ("refresh", None, None),
                        _plan_validation_key(pref_obs),
                        True,
                    )
                    best_plan_path = pref_path + [ref_step]
                    best_plan_label = "then roll"
                    best_pt = pref_pt
        if trace is not None:
            trace.counts["roll_prefixes_scored"] = n_pref_scored
            trace.gates.append(
                GateTrace(
                    "roll_prefixes",
                    n_pref_scored > 0,
                    "%d of %d collected lines that end in a roll kept and scored (stratified, each needs gold for a roll "
                    "plus a purchase)" % (n_pref_scored, trace.counts.get("roll_prefixes", 0)),
                    {
                        "collected": trace.counts.get("roll_prefixes", 0),
                        "kept": len(top_roll_prefixes),
                        "scored": n_pref_scored,
                    },
                )
            )
            trace.value = float(best_plan_score)
            trace.epoch_ev = float(epoch_score)
            trace.margin = margin
            if best_pt is not None:
                best_pt.chosen = True

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
            self._set_queue(player, trimmed, float(best_plan_score), best_plan_label)
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
        particles: SovereignOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
        lobby_id: int = 0,
    ) -> seaglass.TavernAction:
        trace = self._trace
        obs = belief_game.observe()

        # 1. Never end turn with < 7 minions on board while a beneficial playable minion is in hand!
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
                top_pval: Optional[float] = None
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
                        if top_pval is None or pv > top_pval:
                            top_pval = pv
                        if pv > best_pval:
                            best_pval = pv
                            best_play = la
                if trace is not None:
                    if top_pval is None:
                        rs_detail = "playable minion in hand but no legal play to score"
                    else:
                        rs_detail = "stop value %.2f, best play %.2f (needs > stop + 0.5): %s" % (
                            stop_val, top_pval, "plays it" if best_play is not None else "not enough better")
                    trace.gates.append(
                        GateTrace(
                            "rescue_open_play",
                            best_play is not None,
                            rs_detail,
                            {"stop_value": float(stop_val), "best_play_value": top_pval, "needs": 0.5},
                        )
                    )
                if best_play is not None:
                    self._queued_actions.pop(player, None)
                    self.last_note = {"rescue_open_play": 1.0}
                    if trace is not None:
                        trace.finalize_rule = "rescue_open_play"
                    return best_play

        # 2. Freeze-Aware Zero-Waste Invariant (fixes ApexBot Bug #10!):
        #    First check if current shop has a high-EV freeze target (`should_freeze_current_shop`);
        #    if so, Freeze instead of rolling over it! Otherwise spend dead gold on `Refresh`.
        if not obs.is_frozen:
            fz_checked = obs.gold <= 3 and bool(obs.shop)
            fz_value: Optional[float] = None
            fz_fire = False
            if fz_checked:
                fz_value = self.evaluator.freeze_shop_value(obs, ctx=ctx)
                fz_fire = fz_value >= self.evaluator.FREEZE_THRESHOLD
            if trace is not None:
                if fz_checked:
                    fz_detail = "gold %d <= 3 with a shop: freeze value %.2f %s %.2f" % (
                        obs.gold, fz_value, ">=" if fz_fire else "<", self.evaluator.FREEZE_THRESHOLD)
                elif obs.gold > 3:
                    fz_detail = "not checked: gold %d > 3" % obs.gold
                else:
                    fz_detail = "not checked: empty shop"
                trace.gates.append(
                    GateTrace(
                        "smart_freeze",
                        fz_fire,
                        fz_detail,
                        {
                            "frozen": False,
                            "gold": obs.gold,
                            "shop": len(obs.shop),
                            **({"freeze_value": float(fz_value)} if fz_value is not None else {}),
                            "threshold": self.evaluator.FREEZE_THRESHOLD,
                        },
                    )
                )
            if fz_fire:
                matched_fz = self._find_matching_legal(legal, ("toggle_freeze", None, None))
                if matched_fz is not None:
                    self._queued_actions.pop(player, None)
                    self.last_note = {"smart_freeze": float(obs.gold)}
                    if trace is not None:
                        trace.finalize_rule = "smart_freeze"
                    return matched_fz

            ref_act = seaglass.TavernAction.refresh()
            zw_legal = belief_game.is_legal(ref_act)
            zw_free = obs.free_refreshes > 0 and obs.gold >= 1
            zw_gold = obs.turn >= 3 and obs.gold >= obs.refresh_cost + 2
            if trace is not None:
                if not zw_legal:
                    zw_detail = "Refresh is not legal (gold %d, cost %d)" % (obs.gold, obs.refresh_cost)
                elif zw_free:
                    zw_detail = "free refresh available (%d) and gold %d >= 1: rolls" % (obs.free_refreshes, obs.gold)
                elif zw_gold:
                    zw_detail = "turn %d >= 3 and gold %d >= %d (roll %d + 2): rolls" % (
                        obs.turn, obs.gold, obs.refresh_cost + 2, obs.refresh_cost)
                else:
                    zw_detail = (
                        "no free refresh and not (turn >= 3 and gold >= %d (roll %d + 2)): turn %d, gold %d"
                        % (obs.refresh_cost + 2, obs.refresh_cost, obs.turn, obs.gold)
                    )
                trace.gates.append(
                    GateTrace(
                        "zero_waste_refresh",
                        bool(zw_legal and (zw_free or zw_gold)),
                        zw_detail,
                        {
                            "legal": bool(zw_legal),
                            "free_refreshes": obs.free_refreshes,
                            "gold": obs.gold,
                            "turn": obs.turn,
                            "refresh_cost": obs.refresh_cost,
                            "free_condition": bool(zw_free),
                            "gold_condition": bool(zw_gold),
                            "gold_needed": obs.refresh_cost + 2,
                        },
                    )
                )
            if zw_legal and (zw_free or zw_gold):
                matched_ref = self._find_matching_legal(legal, _action_key(ref_act))
                if matched_ref is not None:
                    self._queued_actions.pop(player, None)
                    self.last_note = {"zero_waste_refresh": float(obs.gold)}
                    if trace is not None:
                        trace.finalize_rule = "zero_waste_refresh"
                    return matched_ref
        elif trace is not None:
            trace.gates.append(
                GateTrace(
                    "smart_freeze", False, "shop already frozen",
                    {"frozen": True, "gold": obs.gold, "shop": len(obs.shop)},
                )
            )

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
                    prev_pv = self._queue_meta.get(player, (None, ""))[0]
                    pv = trace.value if trace is not None and trace.value is not None else prev_pv
                    self._set_queue(player, steps[1:], pv, "positioning")
                    self.last_note = {"pos_swaps": float(len(repo_acts))}
                    if trace is not None:
                        trace.finalize_rule = "positioning"
                    return first

        end_act = self._find_matching_legal(legal, ("end_turn", None, None))
        if trace is not None:
            trace.finalize_rule = "end_turn"
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
        """Pick the next action; fills `self.last_trace` and `self.last_note` for this decision only."""
        trace = self._begin_trace(player, lobby.turn)
        try:
            act = self._act_in_lobby_impl(lobby, player, trace)
        except BaseException:
            self._trace = None
            raise
        self._end_trace(trace, act)
        return act

    def _act_in_lobby_impl(
        self, lobby: seaglass.LobbyGame, player: int, trace: DecisionTrace
    ) -> seaglass.TavernAction:
        legal = lobby.legal_actions(player)
        if not legal:
            trace.kind = "only_legal"
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            trace.kind = "only_legal"
            return legal[0]

        obs = lobby.observe(player)
        lobby_id = id(lobby)

        if lobby.duos and self.catalog_name != "duos":
            self.catalog_name = "duos"
            self.sov_kb = get_sovereign_knowledge("duos")
            self.kb = self.sov_kb.base
            self.meta = self.kb.meta
            self.belief = SovereignBeliefTracker(
                catalog_name="duos",
                num_particles=self.belief.num_particles,
                seed=self.rng.getrandbits(31),
            )
            self.evaluator = SovereignEvaluator(
                catalog_name="duos",
                use_physics_rollout=self.evaluator.use_physics_rollout,
                use_neural_value=self.evaluator.use_neural_value,
                neural_weight=self.evaluator.neural_weight,
            )
            self.positioner = self.evaluator.positioner

        if lobby.phase == "hero" or legal[0].kind == "choose_hero":
            self._queued_actions.pop(player, None)
            trace.kind = "hero"
            ctx_hero = lobby.context(player)
            bg = lobby.belief_tavern(player, seed=self.rng.getrandbits(63) | 1, extra_units=[])
            return self.evaluator.choose_best_hero(obs, legal, ctx=ctx_hero, base_game=bg)

        if lobby.duos:
            pass_act = self._check_duos_pass_action(obs, legal, player)
            if pass_act is not None:
                trace.kind = "duos_pass"
                return pass_act

        queued = self._queued_actions.get(player)
        if queued and legal[0].kind not in ("choose_discover", "choose_trinket"):
            next_key, exp_sig, stop_after = queued[0]
            if _plan_validation_key(obs) == exp_sig:
                if next_key[0] != "end_turn":
                    matched = self._find_matching_legal(legal, next_key)
                    if matched is not None:
                        queued.pop(0)
                        self._mark_queued(trace, player, len(queued))
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
                            or (obs.gold <= 3 and bool(obs.shop) and self.evaluator.should_freeze_current_shop(obs))
                        )
                    )
                    and not (
                        len(obs.board) < 7
                        and any(not c.is_spell and not c.unplayable for c in obs.hand)
                    )
                ):
                    self._queued_actions.pop(player, None)
                    end_act = self._find_matching_legal(legal, ("end_turn", None, None))
                    if end_act is not None:
                        self._mark_queued(trace, player, 0)
                        trace.finalize_rule = "end_turn"
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
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            return legal[0]
        k0 = legal[0].kind
        if k0 == "choose_hero":
            return self.evaluator.choose_best_hero(obs, legal, ctx=None, base_game=None)
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
                    tv = self.sov_kb.trinket_prior_score(tname) - 0.8 * tcost
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

        pruned = prune_sovereign_actions(obs, legal, self.evaluator, self.kb)
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

        for a in pruned:
            if a.kind == "play":
                hi = a.args[0]
                if hi is not None and hi < len(obs.hand):
                    hc = obs.hand[hi]
                    if hc.is_spell and not hc.unplayable:
                        return a
                    if not hc.is_spell and not hc.unplayable and len(obs.board) < 7:
                        return a

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

        if obs.turn >= 4:
            dd = self._find_matching_legal(pruned, ("dark_discovery", None, None))
            if dd is not None:
                return dd

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

        if len(obs.board) >= 7 and any(not c.is_spell and not c.unplayable for c in obs.hand):
            for a in pruned:
                if a.kind == "sell":
                    return a

        if self._can_productive_refresh(obs):
            ref = self._find_matching_legal(pruned, ("refresh", None, None))
            if ref is not None:
                return ref

        end_a = self._find_matching_legal(legal, ("end_turn", None, None))
        return end_a if end_a is not None else legal[0]

    def act(self, seat: Any) -> int:
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

        raise TypeError(f"Unsupported seat type for SovereignBot.act: {type(seat)!r}")
