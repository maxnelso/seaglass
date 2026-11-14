"""Stateful Imperfect-Information Opponent & Card-Pool Belief Tracker for SovereignBot."""

from __future__ import annotations

import random
from dataclasses import dataclass
from typing import Dict, List, Optional, Sequence, Tuple

import seaglass

from .apex_knowledge import ApexKnowledge, get_apex_knowledge


@dataclass
class SovereignOpponentParticles:
    opponent_player: int
    tier: int
    boards: List[List[seaglass.Unit]]
    highroll_board: List[seaglass.Unit]
    is_ghost: bool = False
    seen_dt: Optional[int] = None
    has_giant_unit: bool = False
    has_divine_shields: bool = False
    has_summoners: bool = False
    has_cleave_front: bool = False
    colossus_count: int = 0
    max_enemy_hp: int = 0
    lobby_boss_player: int = -1
    lobby_boss_tier: int = 1
    lobby_boss_board: Optional[List[seaglass.Unit]] = None
    lobby_colossus_count: int = 0


class SovereignBeliefTracker:
    """
    Upgraded Imperfect-Information Opponent & Pool Belief Filter for SovereignBot:
      1. Uses ONLY public `LobbyContext` (`tier`, `health`, `armor`, `win_streak`, `seen_board`, `seen_turn`).
      2. Protects Venomous / Leeroy / Morgl / Sin'dorei / Multiplier units from bogus eviction in
         `_evolve_seen_board` (fixing ApexBeliefTracker's scam-deletion bug).
      3. Preserves exact observed slot order on `particles.boards[0]` when `dt <= 1` so `SovereignPositioner`
         can counter-position against enemy Cleaves, Taunts, and Sin'dorei!
      4. Tracks per-opponent multi-sighting empirical stat velocity (`_opp_stat_history`) to forecast
         scaling opponents accurately without hallucinating colossi.
    """

    def __init__(
        self,
        catalog_name: str = "full",
        num_particles: int = 4,
        seed: int = 2029,
    ) -> None:
        self.catalog_name = catalog_name
        self.kb: ApexKnowledge = get_apex_knowledge(catalog_name)
        self.meta = self.kb.meta
        self.num_particles = max(2, num_particles)
        self.rng = random.Random(seed)

        self._protected_multiplier_ids = frozenset(
            cid
            for name in (
                "Titus Rivendare",
                "Brann Bronzebeard",
                "Drakkari Enchanter",
                "Young Murk-Eye",
                "Persistent Poet",
                "Banana Slamma",
                "Lurking Leviathan",
            )
            if (cid := self.kb.name_to_id.get(name)) is not None
        )

        self._lobby_tag: Optional[int] = None
        self._opp_prev_tier: Dict[int, Tuple[int, int]] = {}
        # opp_p -> [(seen_turn_prev, total_stats_prev), (seen_turn_last, total_stats_last)]
        self._opp_stat_history: Dict[int, List[Tuple[int, int]]] = {}
        self._cached_particles_key: Optional[Tuple[int, int, int]] = None
        self._cached_particles: Optional[SovereignOpponentParticles] = None
        self._cached_depletion_key: Optional[Tuple[int, int, int]] = None
        self._cached_depletion: List[seaglass.Unit] = []

    def _sync_lobby(self, lobby_id: int, turn: int) -> None:
        if self._lobby_tag != lobby_id or (
            self._cached_particles_key is not None and turn < self._cached_particles_key[1]
        ):
            self._lobby_tag = lobby_id
            self._opp_prev_tier.clear()
            self._opp_stat_history.clear()
            self._cached_particles_key = None
            self._cached_particles = None
            self._cached_depletion_key = None
            self._cached_depletion = []

    def _record_sighting(self, opp_player: int, seen_turn: int, seen_board: Sequence[seaglass.Unit]) -> None:
        tot_stats = sum(max(0, u.attack) + max(0, u.health) for u in seen_board)
        hist = self._opp_stat_history.setdefault(opp_player, [])
        if not hist:
            hist.append((seen_turn, tot_stats))
        elif hist[-1][0] < seen_turn:
            hist.append((seen_turn, tot_stats))
            if len(hist) > 2:
                del hist[:-2]
        elif hist[-1][0] == seen_turn:
            hist[-1] = (seen_turn, tot_stats)

    def _empirical_unit_step_rate(self, opp_player: int, board_len: int) -> Optional[float]:
        hist = self._opp_stat_history.get(opp_player)
        if not hist or len(hist) < 2:
            return None
        (t1, s1), (t2, s2) = hist[-2], hist[-1]
        dt = t2 - t1
        if dt <= 0:
            return None
        total_rate = max(0.0, float(s2 - s1) / float(dt))
        # Divide across board_len * ~1.35 (atk + hp combined -> single stat budget)
        return min(85.0, total_rate / max(2.0, 1.35 * float(max(3, board_len))))

    def scale_unit_preserving_hooks(
        self,
        base: seaglass.Unit,
        atk_bonus: int,
        hp_bonus: int,
        force_golden: bool = False,
        force_ds: bool = False,
    ) -> seaglass.Unit:
        is_gold = base.is_golden or force_golden
        has_ds = base.divine_shield or force_ds
        if atk_bonus <= 0 and hp_bonus <= 0 and is_gold == base.is_golden and has_ds == base.divine_shield:
            return base

        new_atk = max(1, int(base.attack + atk_bonus))
        new_hp = max(1, int(base.health + hp_bonus))
        slug = self.kb.card_id_to_slug.get(base.card_id)
        if slug is not None:
            tokens = [f"{new_atk}/{new_hp}", f"card:{slug}"]
            if is_gold:
                tokens.append("golden")
            if base.taunt:
                tokens.append("taunt")
            if has_ds:
                tokens.append("divine_shield")
            if base.windfury:
                tokens.append("windfury")
            if base.reborn:
                tokens.append("reborn")
            if base.venomous:
                tokens.append("venomous")
            if base.stealth:
                tokens.append("stealth")
            try:
                return seaglass.parse_unit(" ".join(tokens))
            except ValueError:
                pass

        if has_ds or base.venomous or base.reborn or base.taunt or base.windfury:
            tokens = [f"{new_atk}/{new_hp}"]
            if base.taunt:
                tokens.append("taunt")
            if has_ds:
                tokens.append("divine_shield")
            if base.windfury:
                tokens.append("windfury")
            if base.reborn:
                tokens.append("reborn")
            if base.venomous:
                tokens.append("venomous")
            try:
                return seaglass.parse_unit(" ".join(tokens))
            except ValueError:
                pass

        return seaglass.Unit(
            base.name,
            new_atk,
            new_hp,
            base.tavern_tier,
            base.card_id,
            is_gold,
        )

    def _allowed_templates(
        self, max_tier: int, lobby_tribes: Sequence[str]
    ) -> List[seaglass.CardTemplate]:
        allowed_set = set(lobby_tribes)
        out: List[seaglass.CardTemplate] = []
        capped_t = max(1, min(6, max_tier))
        for t in range(1, capped_t + 1):
            for tmpl in self.meta.templates_by_tier.get(t, ()):
                if not lobby_tribes or tmpl.tribe in ("none", "all"):
                    out.append(tmpl)
                else:
                    prof = self.meta.profile(tmpl.card_id)
                    if prof and any(tr in allowed_set for tr in prof.tribes):
                        out.append(tmpl)
        return out

    @staticmethod
    def _expected_board_size(turn: int, tier: int, leveled_this_turn: bool = False) -> int:
        if turn <= 1:
            return 1
        if turn == 2:
            return 1 if tier >= 2 else 2
        if turn == 3:
            return 2 if (tier >= 3 or leveled_this_turn) else 3
        if turn == 4:
            return 3 if tier >= 4 else 4
        if turn == 5:
            return 5 if tier <= 3 else 4
        if turn == 6:
            return 6
        return 7

    @staticmethod
    def _per_unit_growth_budget(turn: int, tier: int, win_streak: int, highroll: bool = False) -> int:
        if turn <= 3:
            base = max(0, win_streak)
        elif turn <= 6:
            base = (turn - 3) * 4 + max(0, tier - 2) * 3 + win_streak * 2
        elif turn <= 10:
            base = 12 + (turn - 6) * (6 + max(0, tier - 2)) + win_streak * 4
        else:
            late_dt = turn - 10
            base = 44 + int((late_dt ** 1.62) * (9.5 + 2.4 * max(0, tier - 3))) + win_streak * 7
        if highroll:
            base = int(base * 1.45) + max(2, turn - 2)
        return max(0, base)

    def _order_opponent_board(self, board: List[seaglass.Unit]) -> List[seaglass.Unit]:
        def _pos_key(u: seaglass.Unit) -> Tuple[int, int]:
            prof = self.meta.profile(u.card_id)
            if u.card_id == self.kb.sindorei_id:
                return (0, -(u.attack + u.health + 100))
            if prof is not None:
                if prof.has_cleave or prof.has_rally:
                    return (0, -(u.attack + u.health))
                if (prof.has_avenge or prof.is_combat_observer or u.card_id in self._protected_multiplier_ids) and not u.taunt:
                    return (3, u.attack + u.health)
                if prof.is_combat_summoner and not u.taunt:
                    return (1, -(u.attack + u.health))
            if u.taunt:
                return (2, -(u.attack + u.health))
            return (1, -(u.attack + u.health))

        return sorted(board, key=_pos_key)

    def _synthesize_board(
        self,
        turn: int,
        tier: int,
        win_streak: int,
        lobby_tribes: Sequence[str],
        focus_tribe: Optional[str] = None,
        leveled_this_turn: bool = False,
        highroll: bool = False,
    ) -> List[seaglass.Unit]:
        eff_tier = min(6, tier + (1 if highroll and turn >= 7 else 0))
        candidates = self._allowed_templates(eff_tier, lobby_tribes)
        if not candidates:
            s = max(2, 2 * turn)
            return [seaglass.parse_unit(f"{s}/{s}")]

        if focus_tribe is None and lobby_tribes and turn >= 6:
            focus_tribe = self.rng.choice(list(lobby_tribes))

        target_len = self._expected_board_size(turn, tier, leveled_this_turn=leveled_this_turn)
        if highroll and turn >= 5:
            target_len = min(7, target_len + 1)

        weights: List[float] = []
        for tmpl in candidates:
            w = 1.0 + 1.20 * (tmpl.tavern_tier - 1)
            eng_b = self.kb.engine_bonus_by_id.get(tmpl.card_id, 0.0)
            if turn >= 7 and eng_b > 0.0:
                w *= 1.0 + 0.14 * eng_b
            if focus_tribe:
                prof = self.meta.profile(tmpl.card_id)
                if prof and focus_tribe in prof.tribes:
                    w *= 2.9
            weights.append(w)

        chosen = self.rng.choices(candidates, weights=weights, k=target_len)
        budget = self._per_unit_growth_budget(turn, eff_tier, win_streak, highroll=highroll)

        board: List[seaglass.Unit] = []
        for idx, tmpl in enumerate(chosen):
            unit = tmpl.instantiate()
            make_gold = highroll and turn >= 9 and idx == 0
            make_ds = highroll and turn >= 10 and idx == 0 and not unit.divine_shield
            if budget > 0 or make_gold or make_ds:
                slot_mult = 1.35 if idx < 2 else (1.0 if idx < 5 else 0.65)
                b_slot = max(0, int(round(budget * slot_mult)))
                a_add = self.rng.randint(b_slot // 3, max(b_slot // 3, b_slot)) if b_slot > 0 else 0
                h_add = self.rng.randint(b_slot // 3, max(b_slot // 3, b_slot)) if b_slot > 0 else 0
                unit = self.scale_unit_preserving_hooks(
                    unit, a_add, h_add, force_golden=make_gold, force_ds=make_ds
                )
            board.append(unit)
        return self._order_opponent_board(board)

    def _is_protected_seen_unit(self, u: seaglass.Unit) -> bool:
        return (
            u.is_golden
            or u.venomous
            or u.card_id in (self.kb.leeroy_id, self.kb.morgl_id, self.kb.sindorei_id)
            or u.card_id in self._protected_multiplier_ids
            or self.kb.engine_bonus_by_id.get(u.card_id, 0.0) >= 8.0
        )

    def _evolve_seen_board(
        self,
        seen_board: Sequence[seaglass.Unit],
        dt: int,
        turn: int,
        tier: int,
        win_streak: int,
        is_ghost: bool,
        lobby_tribes: Sequence[str],
        highroll: bool = False,
        seen_turn: Optional[int] = None,
        opp_player: int = -1,
        preserve_exact_order: bool = False,
    ) -> List[seaglass.Unit]:
        if is_ghost:
            return list(seen_board)
        if dt <= 0 and not highroll:
            return list(seen_board)

        tribe_counts: Dict[str, int] = {}
        for u in seen_board:
            for tr in u.tribes:
                tribe_counts[tr] = tribe_counts.get(tr, 0) + 1
        focus_tribe = (
            max(tribe_counts.items(), key=lambda kv: kv[1])[0]
            if tribe_counts
            else None
        )

        s_turn = seen_turn if seen_turn is not None else max(1, turn - dt)
        is_uniform_gauntlet, has_exp_seen = self._detect_exponential_board(seen_board, s_turn)
        exp_hp_thresh = max(85, 12 * s_turn - 25)

        base_step_rate = float(max(3, tier + 1 + max(0, turn - 5) * 3 + win_streak * 2))
        emp_rate = self._empirical_unit_step_rate(opp_player, len(seen_board)) if opp_player >= 0 else None
        if emp_rate is not None:
            step_rate = int(round(0.55 * emp_rate + 0.45 * base_step_rate))
        else:
            step_rate = int(round(base_step_rate))
        if highroll:
            step_rate = int(step_rate * 1.45) + 4
        eff_dt = max(1 if highroll else 0, min(dt, 5))

        gauntlet_delta = 0
        if is_uniform_gauntlet and dt > 0:
            p_now = 1 << min(10, max(0, turn - 8))
            p_seen = 1 << min(10, max(0, s_turn - 8))
            gauntlet_delta = min(1200, max(0, 7 * (p_now - p_seen)))

        # When `dt <= 1` and `preserve_exact_order=True`, keep the opponent's exact seen board slots!
        if preserve_exact_order and dt <= 1 and len(seen_board) >= min(7, self._expected_board_size(turn, tier)):
            exact_out: List[seaglass.Unit] = []
            for idx, u in enumerate(seen_board[:7]):
                is_scam_fixed = u.card_id == self.kb.leeroy_id
                if is_scam_fixed:
                    exact_out.append(u)
                    continue
                if is_uniform_gauntlet and dt > 0:
                    atk_add, hp_add = gauntlet_delta, gauntlet_delta
                elif has_exp_seen and (u.health >= exp_hp_thresh or u.health >= 175) and dt > 0:
                    mult = 1.55 * (1.20 if highroll else 1.0)
                    atk_add = max(0, int(round(u.attack * (mult - 1.0))))
                    hp_add = max(0, int(round(u.health * (mult - 1.0))))
                else:
                    carry_mult = 1.20 if idx < 3 else 0.85
                    max_add = max(0, int(round(eff_dt * step_rate * carry_mult)))
                    atk_add = self.rng.randint(max_add // 3, max(max_add // 3, max_add)) if max_add > 0 else 0
                    hp_add = self.rng.randint(max_add // 3, max(max_add // 3, max_add)) if max_add > 0 else 0
                exact_out.append(
                    self.scale_unit_preserving_hooks(u, atk_add, hp_add)
                )
            return exact_out

        # Otherwise rank units while strictly protecting scam / golden / multiplier units from eviction!
        ranked = sorted(
            seen_board,
            key=lambda u: (
                self._is_protected_seen_unit(u),
                u.divine_shield and (u.attack + u.health) >= 18,
                u.attack + u.health + 4 * u.tavern_tier,
            ),
            reverse=True,
        )
        evictable_cnt = sum(1 for u in ranked if not self._is_protected_seen_unit(u))
        raw_rep = 0 if dt <= 1 else min(max(0, len(ranked) - 3), (dt) // 2)
        replacements = min(evictable_cnt, raw_rep)
        kept = ranked[: len(ranked) - replacements] if replacements > 0 else list(ranked)

        target_len = max(len(seen_board), self._expected_board_size(turn, tier))
        needed = max(0, target_len - len(kept))
        fresh = (
            self._synthesize_board(
                turn=turn,
                tier=tier,
                win_streak=win_streak,
                lobby_tribes=lobby_tribes,
                focus_tribe=focus_tribe,
                highroll=highroll,
            )[:needed]
            if needed > 0
            else []
        )

        evolved: List[seaglass.Unit] = []
        for idx, u in enumerate(kept):
            if u.card_id == self.kb.leeroy_id and not is_uniform_gauntlet:
                evolved.append(u)
                continue
            if is_uniform_gauntlet and dt > 0:
                atk_add = gauntlet_delta
                hp_add = gauntlet_delta
            elif has_exp_seen and (u.health >= exp_hp_thresh or u.health >= 175) and dt > 0:
                mult = (1.55 ** min(dt, 3)) * (1.20 if highroll else 1.0)
                atk_add = max(0, int(round(u.attack * (mult - 1.0))))
                hp_add = max(0, int(round(u.health * (mult - 1.0))))
            else:
                carry_mult = 1.25 if idx < 2 else 0.85
                max_add = max(0, int(round(eff_dt * step_rate * carry_mult)))
                atk_add = self.rng.randint(max_add // 3, max(max_add // 3, max_add)) if max_add > 0 else 0
                hp_add = self.rng.randint(max_add // 3, max(max_add // 3, max_add)) if max_add > 0 else 0
            evolved.append(
                self.scale_unit_preserving_hooks(
                    u,
                    atk_add,
                    hp_add,
                    force_golden=(highroll and idx == 0 and turn >= 10),
                )
            )
        evolved.extend(fresh)
        return self._order_opponent_board(evolved[:7])

    @staticmethod
    def _detect_exponential_board(
        seen_board: Optional[Sequence[seaglass.Unit]], seen_turn: Optional[int]
    ) -> Tuple[bool, bool]:
        if not seen_board or seen_turn is None:
            return False, False
        if seen_turn <= 10:
            gaunt_floor = 30
            req_cnt = 6
        elif seen_turn == 11:
            gaunt_floor = 46
            req_cnt = 5
        elif seen_turn == 12:
            gaunt_floor = 95
            req_cnt = 5
        else:
            gaunt_floor = 185
            req_cnt = 5
        is_uniform = (
            seen_turn >= 10
            and len(seen_board) >= req_cnt
            and sum(1 for u in seen_board if u.attack >= gaunt_floor and u.health >= gaunt_floor) >= req_cnt
        )
        thresh = max(85, 12 * seen_turn - 25)
        has_exp = is_uniform or any(u.health >= thresh or u.health >= 175 for u in seen_board)
        return is_uniform, has_exp

    @classmethod
    def _has_verified_exponential_carry(
        cls, seen_board: Optional[Sequence[seaglass.Unit]], seen_turn: Optional[int]
    ) -> bool:
        return cls._detect_exponential_board(seen_board, seen_turn)[1]

    @staticmethod
    def _summarize_threats(
        boards: Sequence[Sequence[seaglass.Unit]],
        meta,
        allow_colossus: bool = False,
    ) -> Tuple[bool, bool, bool, bool, int, int]:
        giant = False
        ds_cnt = 0
        sum_cnt = 0
        cleave_front = False
        col_cnt = 0
        max_hp = 0
        for b in boards:
            local_ds = 0
            local_sum = 0
            local_col = 0
            for slot_i, u in enumerate(b):
                if u.health > max_hp:
                    max_hp = u.health
                if u.health >= 55 or (u.attack >= 55 and u.health >= 35):
                    giant = True
                if allow_colossus and u.health >= 155:
                    local_col += 1
                if u.divine_shield:
                    local_ds += 1
                prof = meta.profile(u.card_id)
                if prof is not None and prof.has_cleave and slot_i <= 1 and u.attack >= 12:
                    cleave_front = True
                if u.reborn or (prof is not None and prof.is_combat_summoner):
                    local_sum += 1
            ds_cnt = max(ds_cnt, local_ds)
            sum_cnt = max(sum_cnt, local_sum)
            col_cnt = max(col_cnt, local_col)
        return giant, (ds_cnt >= 2), (sum_cnt >= 2), cleave_front, col_cnt, max_hp

    def next_opponent_particles(
        self,
        turn: int,
        player: int,
        ctx: Optional[seaglass.LobbyContext],
        lobby_id: int = 0,
    ) -> SovereignOpponentParticles:
        self._sync_lobby(lobby_id, turn)

        if ctx is None or not ctx.opponents:
            tier = min(6, 1 + turn // 2)
            boards = [
                self._synthesize_board(turn, tier, 0, ())
                for _ in range(self.num_particles)
            ]
            hr = self._synthesize_board(turn, tier, 1, (), highroll=True)
            giant, has_ds, has_sum, clv_f, col_cnt, max_hp = self._summarize_threats(boards, self.meta)
            return SovereignOpponentParticles(
                opponent_player=-1,
                tier=tier,
                boards=boards,
                highroll_board=hr,
                is_ghost=False,
                seen_dt=None,
                has_giant_unit=giant,
                has_divine_shields=has_ds,
                has_summoners=has_sum,
                has_cleave_front=clv_f,
                colossus_count=col_cnt,
                max_enemy_hp=max_hp,
            )

        key = (lobby_id, turn, player)
        if self._cached_particles_key == key and self._cached_particles is not None:
            return self._cached_particles

        leveled_map: Dict[int, bool] = {}
        for opp in ctx.opponents:
            prev = self._opp_prev_tier.get(opp.player)
            if prev is not None and prev[0] < turn and opp.tier > prev[1]:
                leveled_map[opp.player] = True
            else:
                leveled_map[opp.player] = False
            self._opp_prev_tier[opp.player] = (turn, opp.tier)
            if opp.seen_turn is not None and opp.seen_board:
                self._record_sighting(opp.player, opp.seen_turn, opp.seen_board)

        next_opp = next((o for o in ctx.opponents if o.is_next), ctx.opponents[0])
        is_ghost = bool(ctx.next_is_ghost or (not next_opp.alive))
        lobby_tribes = list(ctx.tribes)
        opp_leveled = leveled_map.get(next_opp.player, False)
        seen_dt = (
            max(0, turn - next_opp.seen_turn)
            if (next_opp.seen_turn is not None and next_opp.seen_board)
            else None
        )

        next_has_exp = self._has_verified_exponential_carry(next_opp.seen_board, next_opp.seen_turn)

        boards: List[List[seaglass.Unit]] = []
        for p_idx in range(self.num_particles):
            if seen_dt is not None and next_opp.seen_board:
                b = self._evolve_seen_board(
                    seen_board=next_opp.seen_board,
                    dt=seen_dt,
                    turn=turn,
                    tier=max(1, next_opp.tier),
                    win_streak=next_opp.win_streak,
                    is_ghost=is_ghost,
                    lobby_tribes=lobby_tribes,
                    highroll=False,
                    seen_turn=next_opp.seen_turn,
                    opp_player=next_opp.player,
                    preserve_exact_order=(p_idx == 0 and seen_dt <= 1),
                )
            else:
                eff_turn = max(1, turn - 1) if is_ghost else turn
                b = self._synthesize_board(
                    turn=eff_turn,
                    tier=max(1, next_opp.tier),
                    win_streak=0 if is_ghost else next_opp.win_streak,
                    lobby_tribes=lobby_tribes,
                    leveled_this_turn=(opp_leveled and not is_ghost),
                    highroll=False,
                )
            boards.append(b)

        if is_ghost:
            hr_board = boards[0]
        elif seen_dt is not None and next_opp.seen_board:
            hr_board = self._evolve_seen_board(
                seen_board=next_opp.seen_board,
                dt=seen_dt,
                turn=turn,
                tier=max(1, next_opp.tier),
                win_streak=next_opp.win_streak,
                is_ghost=False,
                lobby_tribes=lobby_tribes,
                highroll=True,
                seen_turn=next_opp.seen_turn,
                opp_player=next_opp.player,
            )
        else:
            hr_board = self._synthesize_board(
                turn=turn,
                tier=max(1, next_opp.tier),
                win_streak=next_opp.win_streak + 1,
                lobby_tribes=lobby_tribes,
                highroll=True,
            )

        giant, has_ds, has_sum, clv_f, col_cnt, max_hp = self._summarize_threats(
            boards, self.meta, allow_colossus=next_has_exp
        )
        if turn >= 12 and ctx.alive_units <= 4 and not is_ghost:
            giant = True
        lobby_boss_player = -1
        lobby_boss_tier = max(1, next_opp.tier)
        lobby_boss_board: Optional[List[seaglass.Unit]] = None
        lobby_col_cnt = col_cnt

        if turn >= 9:
            best_boss_strength = -1.0
            for o in ctx.opponents:
                if not o.alive:
                    continue
                if turn >= 13 and o.win_streak >= 4 and (o.seen_turn is None or o.seen_turn <= 9):
                    lobby_col_cnt = max(lobby_col_cnt, 2)
                if o.seen_turn is None or not o.seen_board:
                    continue
                o_has_exp = self._has_verified_exponential_carry(o.seen_board, o.seen_turn)
                if o.player == next_opp.player and not is_ghost:
                    cand_b = boards[0]
                else:
                    o_dt = max(0, turn - o.seen_turn)
                    cand_b = self._evolve_seen_board(
                        seen_board=o.seen_board,
                        dt=o_dt,
                        turn=turn,
                        tier=max(1, o.tier),
                        win_streak=o.win_streak,
                        is_ghost=False,
                        lobby_tribes=lobby_tribes,
                        highroll=False,
                        seen_turn=o.seen_turn,
                        opp_player=o.player,
                    )

                o_giant, o_ds, _, _, o_col, o_max_hp = self._summarize_threats(
                    [cand_b], self.meta, allow_colossus=o_has_exp
                )
                if o_has_exp and o_giant and turn >= 10:
                    giant = True
                if o_has_exp and o_ds and turn >= 10:
                    has_ds = True
                if o_col > lobby_col_cnt:
                    lobby_col_cnt = o_col
                if o_has_exp and o_max_hp > max_hp:
                    max_hp = o_max_hp

                b_strength = sum(
                    u.attack + u.health + (35 if u.divine_shield else 0) + (40 if u.venomous else 0)
                    for u in cand_b
                )
                if b_strength > best_boss_strength:
                    best_boss_strength = b_strength
                    lobby_boss_player = o.player
                    lobby_boss_tier = max(1, o.tier)
                    lobby_boss_board = cand_b

        res = SovereignOpponentParticles(
            opponent_player=next_opp.player,
            tier=max(1, next_opp.tier),
            boards=boards,
            highroll_board=hr_board,
            is_ghost=is_ghost,
            seen_dt=seen_dt,
            has_giant_unit=giant,
            has_divine_shields=has_ds,
            has_summoners=has_sum,
            has_cleave_front=clv_f,
            colossus_count=col_cnt,
            max_enemy_hp=max_hp,
            lobby_boss_player=lobby_boss_player,
            lobby_boss_tier=lobby_boss_tier,
            lobby_boss_board=lobby_boss_board,
            lobby_colossus_count=lobby_col_cnt,
        )
        self._cached_particles_key = key
        self._cached_particles = res
        return res

    def sample_pool_depletion_units(
        self,
        turn: int,
        ctx: Optional[seaglass.LobbyContext],
        player: int = 0,
        lobby_id: int = 0,
    ) -> List[seaglass.Unit]:
        self._sync_lobby(lobby_id, turn)
        if ctx is None:
            return []
        key = (lobby_id, turn, player)
        if self._cached_depletion_key == key:
            return self._cached_depletion

        taken: List[seaglass.Unit] = []
        lobby_tribes = list(ctx.tribes)
        for opp in ctx.opponents:
            if not opp.alive:
                continue
            if opp.seen_board:
                taken.extend(opp.seen_board)
            else:
                synth = self._synthesize_board(
                    turn=turn,
                    tier=max(1, opp.tier),
                    win_streak=opp.win_streak,
                    lobby_tribes=lobby_tribes,
                )
                taken.extend(synth)
        self._cached_depletion_key = key
        self._cached_depletion = taken
        return taken
