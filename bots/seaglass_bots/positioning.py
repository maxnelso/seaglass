"""End-of-Turn Board Positioning Optimizer (Successive Halving + 1v1 Regret Matching)."""

from __future__ import annotations

import itertools
import random
from typing import Dict, List, Optional, Sequence, Tuple

import seaglass

from .belief import OpponentParticleSet
from .catalog_meta import CardMetaDB, get_meta_db
from .evaluator import LeafEvaluator


def _unit_signature(u: seaglass.Unit) -> Tuple:
    return (
        u.card_id,
        u.attack,
        u.health,
        u.is_golden,
        u.taunt,
        u.divine_shield,
        u.windfury,
        u.reborn,
        u.venomous,
        u.stealth,
        u.extra_deathrattles,
        tuple(u.dark_gifts),
    )


class PositioningSolver:
    """Finds optimal board combat ordering via heuristic pruning + Successive Halving."""

    def __init__(
        self,
        catalog_name: str = "full",
        max_candidates: int = 64,
        seed: int = 2026,
        evaluator: Optional[LeafEvaluator] = None,
    ) -> None:
        self.meta: CardMetaDB = get_meta_db(catalog_name)
        self.evaluator = evaluator if evaluator is not None else LeafEvaluator(catalog_name)
        self.max_candidates = max_candidates
        self.rng = random.Random(seed)

    def _layout_prior(self, board: Sequence[seaglass.Unit]) -> float:
        """Fast combat-micro heuristic used to rank the <= 5,040 permutations."""
        n = len(board)
        if n <= 1:
            return 0.0
        score = 0.0
        for i, u in enumerate(board):
            frac = i / max(1, n - 1)  # 0.0 = leftmost attacker, 1.0 = rightmost backline
            prof = self.meta.profile(u.card_id)
            is_cleave = prof.has_cleave if prof else False
            is_rally = prof.has_rally if prof else False
            is_summoner = (prof.is_combat_summoner if prof else False) or u.reborn
            is_backline = (
                (prof.has_avenge or prof.is_combat_observer) if prof else False
            )

            # Cleave, Rally, and high-attack Windfury want to swing early before dying.
            if is_cleave or is_rally:
                score += 4.5 * (1.0 - frac)
            if u.windfury and u.attack >= 6:
                score += 2.0 * (1.0 - frac)

            # Avenge & combat observers belong in the protected backline (rightmost slots) if untaunted.
            if is_backline and not u.taunt:
                score += 4.8 * frac

            # Summoners prefer attacking after a bumper (slot 1..3) and before backline observers.
            if is_summoner and not is_backline:
                if n == 7 and i == 0 and u.health > 4:
                    score -= 0.8  # Prefer a disposable bumper dying first on a full 7-board.
                score += 2.2 * (1.0 - abs(frac - 0.35))

            # Unshielded Venomous prefers slot 1..3 behind a shield-popper rather than slot 0.
            if u.venomous and not u.divine_shield and i == 0 and n >= 3:
                score -= 2.2

            # Anti-cleave Taunt positioning: keep Taunts on edges or next to disposable/shielded units,
            # never sandwiched between two fragile untaunted backline engines!
            if u.taunt:
                if i == 0 or i == n - 1:
                    score += 1.6  # Edge taunt cuts cleave splash in half!
                neighbors = []
                if i > 0:
                    neighbors.append(board[i - 1])
                if i + 1 < n:
                    neighbors.append(board[i + 1])
                for nb in neighbors:
                    nb_prof = self.meta.profile(nb.card_id)
                    if nb_prof and (nb_prof.has_avenge or nb_prof.is_combat_observer):
                        score -= 4.0  # Never expose Titus/Avenge engine next to Taunt!
                    elif nb.venomous and not nb.divine_shield:
                        score -= 2.5
        return score

    def _candidate_permutations(
        self, board: Sequence[seaglass.Unit]
    ) -> List[Tuple[int, ...]]:
        n = len(board)
        if n <= 1:
            return [tuple(range(n))]

        identity = tuple(range(n))
        seen_sigs: Dict[Tuple, Tuple[int, ...]] = {}
        sigs = [_unit_signature(u) for u in board]

        for perm in itertools.permutations(range(n)):
            board_sig = tuple(sigs[i] for i in perm)
            if board_sig not in seen_sigs:
                seen_sigs[board_sig] = perm

        unique_perms = list(seen_sigs.values())
        if len(unique_perms) <= self.max_candidates:
            return unique_perms

        scored = sorted(
            unique_perms,
            key=lambda p: self._layout_prior([board[i] for i in p]),
            reverse=True,
        )
        top = scored[: self.max_candidates]
        if identity not in top:
            top[-1] = identity
        return top

    def _combat_score(
        self,
        tavern: seaglass.TavernGame,
        cand_board: List[seaglass.Unit],
        particles: OpponentParticleSet,
        ctx: Optional[seaglass.LobbyContext],
        combats_per_particle: int,
        seed_offset: int,
    ) -> float:
        obs = tavern.observe()
        hp_now = float(obs.health + obs.armor)
        alive_units = ctx.alive_units if ctx is not None else 8
        damage_cap = ctx.damage_cap if ctx is not None else None
        lobby_hps = (
            [o.health + o.armor for o in ctx.opponents if o.alive]
            if ctx is not None
            else [30] * 7
        )

        total = 0.0
        for idx, opp_board in enumerate(particles.boards):
            dist = tavern.equity_against(
                opp_board,
                opponent_tier=particles.tier,
                n=combats_per_particle,
                base_seed=obs.turn * 4999 + idx * 131 + seed_offset,
                board=cand_board,
            )
            if dist.b_win_rate > 1e-6:
                loss = float(max(0, -dist.damage_min))
                if damage_cap is not None:
                    loss = min(loss, float(damage_cap))
                avg_loss = 0.65 * loss
            else:
                loss = 0.0
                avg_loss = 0.0

            u_safe = self.evaluator.icm_survival_utility(hp_now, alive_units, lobby_hps)
            u_loss = 0.7 * self.evaluator.icm_survival_utility(
                hp_now - avg_loss, alive_units, lobby_hps
            ) + 0.3 * self.evaluator.icm_survival_utility(
                hp_now - loss, alive_units, lobby_hps
            )
            score = (
                (dist.a_win_rate + dist.draw_rate) * u_safe
                + dist.b_win_rate * u_loss
                + 2.5 * dist.a_win_rate
                + 0.15 * dist.damage_mean
            )
            total += score
        return total / max(1, len(particles.boards))

    def _solve_1v1_regret_matching(
        self,
        tavern: seaglass.TavernGame,
        my_perms: Sequence[Tuple[int, ...]],
        opp_board: Sequence[seaglass.Unit],
        opp_tier: int,
        ctx: Optional[seaglass.LobbyContext],
    ) -> Tuple[int, ...]:
        """Compute mixed Nash strategy via Regret Matching for 1v1 heads-up counter-positioning."""
        obs = tavern.observe()
        my_cands = list(my_perms[:6])
        opp_perms = self._candidate_permutations(opp_board)[:6]
        if len(my_cands) <= 1 or len(opp_perms) <= 1:
            return my_cands[0]

        m = len(my_cands)
        k = len(opp_perms)
        payoff = [[0.0] * k for _ in range(m)]
        for i, mp in enumerate(my_cands):
            mb = [obs.board[idx] for idx in mp]
            for j, op in enumerate(opp_perms):
                ob = [opp_board[idx] for idx in op]
                pset = OpponentParticleSet(opponent_player=-1, tier=opp_tier, boards=[ob])
                payoff[i][j] = self._combat_score(
                    tavern, mb, pset, ctx, combats_per_particle=24, seed_offset=701 + i * 17 + j
                )

        regret_my = [0.0] * m
        regret_opp = [0.0] * k
        strat_sum_my = [0.0] * m

        def _regret_to_strat(regrets: List[float]) -> List[float]:
            pos = [max(0.0, r) for r in regrets]
            s = sum(pos)
            if s <= 1e-9:
                return [1.0 / len(regrets)] * len(regrets)
            return [p / s for p in pos]

        for _ in range(120):
            p_my = _regret_to_strat(regret_my)
            p_opp = _regret_to_strat(regret_opp)
            for i in range(m):
                strat_sum_my[i] += p_my[i]

            # Expected game value under current strategies.
            u_my_action = [
                sum(p_opp[j] * payoff[i][j] for j in range(k)) for i in range(m)
            ]
            u_game = sum(p_my[i] * u_my_action[i] for i in range(m))
            for i in range(m):
                regret_my[i] += u_my_action[i] - u_game

            u_opp_action = [
                sum(p_my[i] * (-payoff[i][j]) for i in range(m)) for j in range(k)
            ]
            for j in range(k):
                regret_opp[j] += u_opp_action[j] - (-u_game)

        total_w = sum(strat_sum_my)
        avg_strat = (
            [w / total_w for w in strat_sum_my]
            if total_w > 1e-9
            else [1.0 / m] * m
        )
        return self.rng.choices(my_cands, weights=avg_strat, k=1)[0]

    def best_permutation(
        self,
        tavern: seaglass.TavernGame,
        particles: OpponentParticleSet,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> Tuple[int, ...]:
        """Run Successive Halving (and 1v1 Regret Matching in heads-up) to pick board order."""
        obs = tavern.observe()
        board = obs.board
        n = len(board)
        if n <= 1:
            return tuple(range(n))

        candidates = self._candidate_permutations(board)
        if len(candidates) == 1:
            return candidates[0]

        # Successive Halving schedule: (keep_fraction, combats_per_particle)
        rounds = [(0.25, 8), (0.35, 24), (1.0, 64)]
        survivors = candidates
        scored: List[Tuple[float, Tuple[int, ...]]] = []

        for r_idx, (keep_frac, n_sims) in enumerate(rounds):
            scored = []
            for perm in survivors:
                cand_board = [board[i] for i in perm]
                val = self._combat_score(
                    tavern,
                    cand_board,
                    particles,
                    ctx,
                    combats_per_particle=n_sims,
                    seed_offset=100 * (r_idx + 1),
                )
                # Tiny tie-break toward combat micro prior so equal-equity boards stay clean.
                val += 1e-3 * self._layout_prior(cand_board)
                scored.append((val, perm))
            scored.sort(key=lambda x: x[0], reverse=True)
            if keep_frac < 1.0 and len(scored) > 2:
                keep_n = max(2, int(round(len(scored) * keep_frac)))
                survivors = [p for _, p in scored[:keep_n]]

        # If board has End-of-Turn or persistent combat triggers (`Young Murk-Eye`, `Bannerboar`,
        # `Rylak Metalhead`, `Persistent Poet`), re-rank top 5 layouts with full 1-turn physics!
        has_positional_persistence = any(
            (prof := self.meta.profile(u.card_id)) is not None
            and (prof.has_end_of_turn or prof.has_deathrattle or prof.is_scaling_engine)
            for u in board
        )
        if self.evaluator.use_physics_rollout and has_positional_persistence and len(scored) > 1:
            phys_scored: List[Tuple[float, Tuple[int, ...]]] = []
            for _, perm in scored[:5]:
                sim = tavern.copy()
                for act in self.permutation_to_actions(perm):
                    if sim.is_legal(act):
                        sim.step(act)
                full_v = self.evaluator.evaluate_turn_end(
                    sim, particles, ctx=ctx, combats_per_particle=24
                )
                phys_scored.append((full_v, perm))
            phys_scored.sort(key=lambda x: x[0], reverse=True)
            scored = phys_scored + scored[5:]

        # In 1v1 finals (`alive_units == 2`) with a known opponent board, use Regret Matching
        # over the top finalist layouts so the bot cannot be counter-positioned!
        if (
            ctx is not None
            and ctx.alive_units == 2
            and not ctx.next_is_ghost
            and particles.boards
            and len(particles.boards[0]) >= 3
        ):
            top_perms = [p for _, p in scored[:6]]
            return self._solve_1v1_regret_matching(
                tavern, top_perms, particles.boards[0], particles.tier, ctx
            )

        return scored[0][1]

    @staticmethod
    def permutation_to_actions(perm: Sequence[int]) -> List[seaglass.TavernAction]:
        """Convert target permutation of current indices `0..n` into `Reposition` actions."""
        n = len(perm)
        cur = list(range(n))
        actions: List[seaglass.TavernAction] = []
        for target_slot in range(n):
            want = perm[target_slot]
            cur_pos = cur.index(want)
            if cur_pos != target_slot:
                actions.append(seaglass.TavernAction.reposition(cur_pos, target_slot))
                moved = cur.pop(cur_pos)
                cur.insert(target_slot, moved)
        return actions
