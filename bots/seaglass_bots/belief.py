"""Opponent warband & shared CardPool Particle Filter (`LobbyBeliefFilter`)."""

from __future__ import annotations

import random
from dataclasses import dataclass
from typing import Dict, List, Optional, Sequence, Tuple

import seaglass

from .catalog_meta import CardMetaDB, get_meta_db


@dataclass
class OpponentParticleSet:
    opponent_player: int
    tier: int
    boards: List[List[seaglass.Unit]]


class LobbyBeliefFilter:
    """Sequential Monte Carlo belief filter over unseen opponent boards and pool copies."""

    def __init__(
        self,
        catalog_name: str = "full",
        num_particles: int = 4,
        seed: int = 1337,
    ) -> None:
        self.meta: CardMetaDB = get_meta_db(catalog_name)
        self.num_particles = max(1, num_particles)
        self.rng = random.Random(seed)
        self._cached_turn: int = -1
        self._cached_player: int = -1
        self._next_particles: Optional[OpponentParticleSet] = None

    def _allowed_templates(
        self, max_tier: int, lobby_tribes: Sequence[str]
    ) -> List[seaglass.CardTemplate]:
        allowed_set = set(lobby_tribes)
        out: List[seaglass.CardTemplate] = []
        for t in range(1, max(1, min(6, max_tier)) + 1):
            for tmpl in self.meta.templates_by_tier.get(t, ()):
                if not lobby_tribes or tmpl.tribe in ("none", "all"):
                    out.append(tmpl)
                else:
                    prof = self.meta.profile(tmpl.card_id)
                    if prof and any(tr in allowed_set for tr in prof.tribes):
                        out.append(tmpl)
        return out

    def _scale_unit(
        self,
        base: seaglass.Unit,
        atk_bonus: int,
        hp_bonus: int,
    ) -> seaglass.Unit:
        if atk_bonus <= 0 and hp_bonus <= 0:
            return base
        # Preserve keywords on shield/taunt/venomous/reborn units when stats are modest;
        # otherwise instantiate scaled stats while preserving card_id hooks.
        if base.divine_shield or base.venomous or base.reborn or base.taunt:
            if atk_bonus + hp_bonus <= 4:
                return base
            kw: List[str] = []
            if base.taunt:
                kw.append("taunt")
            if base.divine_shield:
                kw.append("divine_shield")
            if base.windfury:
                kw.append("windfury")
            if base.reborn:
                kw.append("reborn")
            if base.venomous:
                kw.append("venomous")
            spec = f"{max(1, base.attack + atk_bonus)}/{max(1, base.health + hp_bonus)}"
            if kw:
                spec += " " + " ".join(kw)
            try:
                return seaglass.parse_unit(spec)
            except ValueError:
                pass
        return seaglass.Unit(
            base.name,
            max(1, base.attack + atk_bonus),
            max(1, base.health + hp_bonus),
            base.tavern_tier,
            base.card_id,
            base.is_golden,
        )

    def _expected_board_size(self, turn: int, tier: int) -> int:
        if turn <= 1:
            return 1
        if turn == 2:
            return 1 if tier >= 2 else 2
        if turn == 3:
            return 3 if tier <= 2 else 2
        if turn == 4:
            return 4
        if turn == 5:
            return 5
        if turn == 6:
            return 6
        return 7

    def _synthesize_board(
        self,
        turn: int,
        tier: int,
        win_streak: int,
        lobby_tribes: Sequence[str],
        focus_tribe: Optional[str] = None,
    ) -> List[seaglass.Unit]:
        candidates = self._allowed_templates(tier, lobby_tribes)
        if not candidates:
            return [seaglass.parse_unit(f"{2 * turn}/{2 * turn}")]

        target_len = self._expected_board_size(turn, tier)
        # Weight higher-tier and tribe-coherent templates more strongly on later turns.
        weights: List[float] = []
        for tmpl in candidates:
            w = 1.0 + 0.85 * (tmpl.tavern_tier - 1)
            if focus_tribe:
                prof = self.meta.profile(tmpl.card_id)
                if prof and focus_tribe in prof.tribes:
                    w *= 2.4
            weights.append(w)

        chosen = self.rng.choices(candidates, weights=weights, k=target_len)
        growth = max(0, turn - 4) * (1 + win_streak // 2) + max(0, tier - 2)
        board: List[seaglass.Unit] = []
        for idx, tmpl in enumerate(chosen):
            unit = tmpl.instantiate()
            if growth > 0:
                a_add = self.rng.randint(0, growth)
                h_add = self.rng.randint(0, growth)
                unit = self._scale_unit(unit, a_add, h_add)
            board.append(unit)
        # Place taunts near edges and high-attack units near front as a realistic opponent prior.
        board.sort(
            key=lambda u: (
                0 if (u.windfury or not u.taunt) else 1,
                -(u.attack + u.health),
            )
        )
        return board

    def _evolve_seen_board(
        self,
        seen_board: Sequence[seaglass.Unit],
        dt: int,
        turn: int,
        tier: int,
        win_streak: int,
        is_ghost: bool,
        lobby_tribes: Sequence[str],
    ) -> List[seaglass.Unit]:
        if is_ghost or dt <= 0:
            return list(seen_board)

        # Infer opponent's dominant tribe from their last seen warband.
        tribe_counts: Dict[str, int] = {}
        for u in seen_board:
            for tr in u.tribes:
                tribe_counts[tr] = tribe_counts.get(tr, 0) + 1
        focus_tribe = (
            max(tribe_counts.items(), key=lambda kv: kv[1])[0]
            if tribe_counts
            else None
        )

        # Keep strongest core units; replace weak early-tier units over elapsed turns `dt`.
        ranked = sorted(
            seen_board,
            key=lambda u: (
                u.is_golden,
                u.attack + u.health + 3 * u.tavern_tier,
            ),
            reverse=True,
        )
        replacements = min(len(ranked), max(1, dt))
        kept = ranked[: max(0, len(ranked) - replacements)]

        target_len = max(len(seen_board), self._expected_board_size(turn, tier))
        needed = max(0, target_len - len(kept))
        fresh = self._synthesize_board(
            turn=turn,
            tier=tier,
            win_streak=win_streak,
            lobby_tribes=lobby_tribes,
            focus_tribe=focus_tribe,
        )[:needed]

        per_turn_buff = max(1, tier // 2 + win_streak // 2)
        evolved: List[seaglass.Unit] = []
        for u in kept:
            atk_add = self.rng.randint(0, dt * per_turn_buff)
            hp_add = self.rng.randint(0, dt * per_turn_buff)
            evolved.append(self._scale_unit(u, atk_add, hp_add))
        evolved.extend(fresh)
        return evolved[:7]

    def next_opponent_particles(
        self,
        turn: int,
        player: int,
        ctx: Optional[seaglass.LobbyContext],
    ) -> OpponentParticleSet:
        """Return `num_particles` warband particles for player `p`'s upcoming combat."""
        if ctx is None or not ctx.opponents:
            # Standalone single-tavern fallback curve.
            tier = min(6, 1 + turn // 2)
            boards = [
                self._synthesize_board(turn, tier, 0, ())
                for _ in range(self.num_particles)
            ]
            return OpponentParticleSet(opponent_player=-1, tier=tier, boards=boards)

        if self._cached_turn == turn and self._cached_player == player and self._next_particles:
            return self._next_particles

        next_opp = next((o for o in ctx.opponents if o.is_next), ctx.opponents[0])
        is_ghost = ctx.next_is_ghost or (not next_opp.alive)
        lobby_tribes = list(ctx.tribes)

        boards: List[List[seaglass.Unit]] = []
        for _ in range(self.num_particles):
            if next_opp.seen_turn is not None and next_opp.seen_board:
                dt = max(0, turn - next_opp.seen_turn)
                b = self._evolve_seen_board(
                    seen_board=next_opp.seen_board,
                    dt=dt,
                    turn=turn,
                    tier=next_opp.tier,
                    win_streak=next_opp.win_streak,
                    is_ghost=is_ghost,
                    lobby_tribes=lobby_tribes,
                )
            else:
                eff_turn = max(1, turn - 1) if is_ghost else turn
                b = self._synthesize_board(
                    turn=eff_turn,
                    tier=max(1, next_opp.tier),
                    win_streak=0 if is_ghost else next_opp.win_streak,
                    lobby_tribes=lobby_tribes,
                )
            boards.append(b)

        res = OpponentParticleSet(
            opponent_player=next_opp.player,
            tier=max(1, next_opp.tier),
            boards=boards,
        )
        self._cached_turn = turn
        self._cached_player = player
        self._next_particles = res
        return res

    def sample_pool_depletion_units(
        self,
        turn: int,
        ctx: Optional[seaglass.LobbyContext],
    ) -> List[seaglass.Unit]:
        """Sample hidden units held by living opponents to deplete `belief_tavern` pool."""
        if ctx is None:
            return []
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
        return taken
