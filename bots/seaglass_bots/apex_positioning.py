"""Apex End-of-Turn & Combat Board Positioning Optimizer (`ApexPositioner`)."""

from __future__ import annotations

import itertools
from typing import Dict, List, Optional, Sequence, Tuple

import seaglass

from .apex_belief import ApexOpponentParticles
from .apex_evaluator import ApexEvaluator
from .apex_knowledge import ApexKnowledge, get_apex_knowledge


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


class ApexPositioner:
    """
    Finds optimal board ordering via:
      1. Engine-exact adjacency & combat-micro layout prior (`Young Murk-Eye`, `Persistent Poet`,
         `Stitched Salvager`, `Leviathan` < `Banana Slamma`, `Al'Akir`, `Sin'dorei`, Cleaves, Taunt splash),
      2. Successive Halving Monte Carlo Combat Rollouts,
      3. Full 1-Turn Rust Physics verification whenever EoT/post-combat positional triggers exist,
      4. Deterministic minimax/best-response counter-positioning in 1v1 finals.
    """

    def __init__(
        self,
        catalog_name: str = "full",
        max_candidates: int = 48,
        evaluator: Optional[ApexEvaluator] = None,
    ) -> None:
        self.kb: ApexKnowledge = get_apex_knowledge(catalog_name)
        self.meta = self.kb.meta
        self.evaluator = evaluator if evaluator is not None else ApexEvaluator(catalog_name)
        self.max_candidates = max_candidates

        self._fragile_backline_ids = frozenset(
            cid
            for name in (
                "Titus Rivendare",
                "Brann Bronzebeard",
                "Drakkari Enchanter",
                "Banana Slamma",
                "Lurking Leviathan",
                "Young Murk-Eye",
            )
            if (cid := self.kb.name_to_id.get(name)) is not None
        )
        self._eot_positional_trinket_ids = frozenset(
            tid
            for tid, (tname, _, _, _) in self.kb.trinket_info.items()
            if tname
            in (
                "Murky Sticker",
                "Young Murk-Eye Sticker",
                "Accord-o-Tron Portrait",
                "Wolfhead Flail",
                "Emergency Gearblade",
            )
        )

    def _precompute_unit_features(self, board: Sequence[seaglass.Unit]) -> List[Tuple]:
        """Precompute per-unit layout attributes once so 5,040 permutations score in <2ms."""
        feats: List[Tuple] = []
        profiles = self.meta.profiles
        kb = self.kb
        fragile_ids = self._fragile_backline_ids
        targeted_bc = kb.targeted_battlecry_ids
        eng_map = kb.engine_bonus_by_id
        poet_id = kb.persistent_poet_id
        salv_id = kb.stitched_salvager_id
        murk_id = kb.young_murk_eye_id
        lev_id = kb.lurking_leviathan_id
        slamma_id = kb.banana_slamma_id
        sin_id = kb.sindorei_id
        leeroy_id = kb.leeroy_id

        for u in board:
            cid = u.card_id
            prof = profiles.get(cid)
            is_cleave = prof.has_cleave if prof else False
            is_rally = prof.has_rally if prof else False
            is_summoner = (prof.is_combat_summoner if prof else False) or u.reborn
            is_obs = (prof.has_avenge or prof.is_combat_observer) if prof else False
            is_fragile = cid in fragile_ids
            is_backline = is_fragile or is_obs
            is_nontarget_bc = bool(prof and prof.has_battlecry and cid not in targeted_bc)
            bc_bonus = 14.0 + 0.6 * eng_map.get(cid, 4.0) if is_nontarget_bc else 0.0
            is_dragon_nb = ("dragon" in u.tribes or "all" in u.tribes) and cid != poet_id
            dragon_bonus = (12.5 + (4.0 if not u.divine_shield else 1.5)) if is_dragon_nb else 0.0
            has_dr = prof.has_deathrattle if prof else False
            salv_victim_val = (
                -20.0
                if (cid == salv_id or is_fragile)
                else ((12.0 if has_dr else 4.0) + 0.12 * min(120, u.attack + u.health))
            )
            feats.append(
                (
                    cid,
                    u.attack,
                    u.health,
                    u.is_golden,
                    u.taunt,
                    u.divine_shield,
                    u.windfury,
                    u.venomous or cid == leeroy_id,
                    is_cleave,
                    is_rally,
                    is_summoner,
                    is_backline,
                    is_fragile or is_obs,
                    bc_bonus,
                    dragon_bonus,
                    salv_victim_val,
                    cid == murk_id,
                    cid == poet_id,
                    cid == salv_id,
                    cid == lev_id,
                    cid == slamma_id,
                    cid == sin_id,
                )
            )
        return feats

    @staticmethod
    def _score_perm_from_feats(
        perm: Tuple[int, ...],
        feats: List[Tuple],
        is_alakir: bool,
        is_illidan: bool,
        enemy_ds: bool,
        enemy_giant: bool,
    ) -> float:
        n = len(perm)
        if n <= 1:
            return 0.0
        denom = float(n - 1)
        score = 0.0
        lev_pos = -1
        slamma_pos = -1

        for i in range(n):
            f = feats[perm[i]]
            (
                _cid,
                atk,
                hp,
                is_gold,
                taunt,
                ds,
                wf,
                is_scam,
                is_cleave,
                is_rally,
                is_summoner,
                is_backline,
                is_splash_vuln,
                _bc_b,
                _drg_b,
                _salv_v,
                is_murk,
                is_poet,
                is_salv,
                is_lev,
                is_slamma,
                is_sin,
            ) = f
            frac = i / denom
            if is_lev:
                lev_pos = i
            elif is_slamma:
                slamma_pos = i

            if is_murk:
                if i > 0:
                    score += feats[perm[i - 1]][13]
                if i + 1 < n:
                    score += feats[perm[i + 1]][13]

            if is_poet:
                if i > 0:
                    score += feats[perm[i - 1]][14]
                if i + 1 < n:
                    score += feats[perm[i + 1]][14]

            if is_salv:
                if i == 0:
                    score -= 18.0
                else:
                    score += feats[perm[i - 1]][15]
                    if is_gold and i + 1 < n:
                        score += feats[perm[i + 1]][15]

            if i == 0 and is_alakir:
                if is_cleave or (atk >= 8 and not ds):
                    score += 11.0
                elif is_backline:
                    score -= 12.0
            if (i == 0 or i == n - 1) and is_illidan:
                if is_cleave or is_rally or atk >= 10:
                    score += 7.0

            if is_sin:
                score += 8.5 * (1.0 - frac)
            elif is_cleave or is_rally:
                score += 5.2 * (1.0 - frac)
            if wf and atk >= 8:
                score += 2.2 * (1.0 - frac)

            if is_scam:
                if enemy_ds and not ds and i == 0 and n >= 3:
                    score -= 4.5
                elif enemy_giant and 1 <= i <= 3:
                    score += 3.5

            if is_backline and not taunt and not is_murk:
                score += 5.5 * frac

            if is_summoner and not is_backline:
                if n == 7 and i == 0 and hp > 6:
                    score -= 1.0
                score += 2.5 * (1.0 - abs(frac - 0.32))

            if taunt:
                if i == 0 or i == n - 1:
                    score += 1.8
                if i > 0:
                    nb_l = feats[perm[i - 1]]
                    if nb_l[12]:
                        score -= 5.0
                    elif nb_l[7] and not nb_l[5]:
                        score -= 3.0
                if i + 1 < n:
                    nb_r = feats[perm[i + 1]]
                    if nb_r[12]:
                        score -= 5.0
                    elif nb_r[7] and not nb_r[5]:
                        score -= 3.0

        if lev_pos >= 0 and slamma_pos >= 0:
            score += 8.5 if lev_pos < slamma_pos else -6.0

        return score

    def layout_prior(
        self,
        board: Sequence[seaglass.Unit],
        obs: Optional[seaglass.TavernObservation] = None,
        particles: Optional[ApexOpponentParticles] = None,
    ) -> float:
        n = len(board)
        if n <= 1:
            return 0.0
        feats = self._precompute_unit_features(board)
        hero_name = self.kb.hero_names.get(obs.hero_id, "") if obs is not None else ""
        enemy_ds = particles.has_divine_shields if particles is not None else False
        enemy_giant = particles.has_giant_unit if particles is not None else False
        return self._score_perm_from_feats(
            tuple(range(n)),
            feats,
            hero_name == "Al'Akir",
            hero_name == "Illidan Stormrage",
            enemy_ds,
            enemy_giant,
        )

    def _candidate_permutations(
        self,
        board: Sequence[seaglass.Unit],
        obs: Optional[seaglass.TavernObservation] = None,
        particles: Optional[ApexOpponentParticles] = None,
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
        if len(unique_perms) <= 1:
            return unique_perms

        feats = self._precompute_unit_features(board)
        hero_name = self.kb.hero_names.get(obs.hero_id, "") if obs is not None else ""
        is_alakir = hero_name == "Al'Akir"
        is_illidan = hero_name == "Illidan Stormrage"
        enemy_ds = particles.has_divine_shields if particles is not None else False
        enemy_giant = particles.has_giant_unit if particles is not None else False

        scored = sorted(
            unique_perms,
            key=lambda p: self._score_perm_from_feats(
                p, feats, is_alakir, is_illidan, enemy_ds, enemy_giant
            ),
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
        particles: ApexOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
        combats_per_particle: int,
        seed_offset: int,
    ) -> float:
        obs = tavern.observe()
        hp_now = self.evaluator.effective_hp(obs)
        alive_units = ctx.alive_units if ctx is not None else 8
        damage_cap = ctx.damage_cap if ctx is not None else None
        lobby_hps = (
            [o.health + o.armor for o in ctx.opponents if o.alive]
            if ctx is not None
            else [30] * 7
        )
        u_safe = self.evaluator.cvar_survival_utility(
            hp_now, alive_units, lobby_hps, damage_cap
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
                u_loss = 0.68 * self.evaluator.cvar_survival_utility(
                    hp_now - avg_loss, alive_units, lobby_hps, damage_cap
                ) + 0.32 * self.evaluator.cvar_survival_utility(
                    hp_now - loss, alive_units, lobby_hps, damage_cap
                )
            else:
                u_loss = u_safe

            score = (
                (dist.a_win_rate + dist.draw_rate) * u_safe
                + dist.b_win_rate * u_loss
                + 3.0 * dist.a_win_rate
                + 0.22 * dist.damage_mean
            )
            total += score
        return total / max(1, len(particles.boards))

    def _has_eot_or_persistent_adjacency(
        self, board: Sequence[seaglass.Unit], obs: Optional[seaglass.TavernObservation] = None
    ) -> bool:
        if obs is not None and any(tid in self._eot_positional_trinket_ids for tid in obs.trinkets):
            return True
        for u in board:
            cid = u.card_id
            if cid in (
                self.kb.young_murk_eye_id,
                self.kb.persistent_poet_id,
                self.kb.stitched_salvager_id,
                self.kb.parasitic_fleshling_id,
            ):
                return True
            prof = self.meta.profiles.get(cid)
            if prof is not None and prof.has_end_of_turn:
                return True
        return False

    def best_permutation(
        self,
        tavern: seaglass.TavernGame,
        particles: ApexOpponentParticles,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> Tuple[int, ...]:
        obs = tavern.observe()
        board = obs.board
        n = len(board)
        if n <= 1:
            return tuple(range(n))

        candidates = self._candidate_permutations(board, obs=obs, particles=particles)
        if len(candidates) == 1:
            return candidates[0]

        has_eot_adj = self._has_eot_or_persistent_adjacency(board, obs=obs)
        prior_elites = candidates[:4] if has_eot_adj else []

        feats = self._precompute_unit_features(board)
        hero_name = self.kb.hero_names.get(obs.hero_id, "")
        is_alakir = hero_name == "Al'Akir"
        is_illidan = hero_name == "Illidan Stormrage"
        enemy_ds = particles.has_divine_shields
        enemy_giant = particles.has_giant_unit

        rounds = [(0.28, 6), (0.35, 18), (1.0, 42)]
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
                prior_w = 0.18 if has_eot_adj else 2e-3
                val += prior_w * self._score_perm_from_feats(
                    perm, feats, is_alakir, is_illidan, enemy_ds, enemy_giant
                )
                scored.append((val, perm))
            scored.sort(key=lambda x: x[0], reverse=True)
            if keep_frac < 1.0 and len(scored) > 2:
                keep_n = max(2, int(round(len(scored) * keep_frac)))
                survivors = [p for _, p in scored[:keep_n]]

        if self.evaluator.use_physics_rollout and has_eot_adj:
            pool_perms: List[Tuple[int, ...]] = []
            for p in prior_elites:
                if p not in pool_perms:
                    pool_perms.append(p)
            for _, p in scored[:5]:
                if p not in pool_perms:
                    pool_perms.append(p)

            phys_scored: List[Tuple[float, Tuple[int, ...]]] = []
            for perm in pool_perms[:7]:
                sim = tavern.copy()
                for act in self.permutation_to_actions(perm):
                    if sim.is_legal(act):
                        sim.step(act)
                full_v = self.evaluator.evaluate_turn_end(
                    sim, particles, ctx=ctx, combats_per_particle=18
                )
                phys_scored.append((full_v, perm))
            phys_scored.sort(key=lambda x: x[0], reverse=True)
            scored = phys_scored

        # Deterministic 1v1 counter-positioning check against opponent's seen/expected order + top swaps!
        if (
            ctx is not None
            and ctx.alive_units == 2
            and not particles.is_ghost
            and particles.boards
            and len(particles.boards[0]) >= 3
            and not has_eot_adj
        ):
            top_my = [p for _, p in scored[:4]]
            opp_base = particles.boards[0]
            opp_id = tuple(range(len(opp_base)))
            opp_heur = [p for p in self._candidate_permutations(opp_base) if p != opp_id]
            opp_cands = [opp_id] + opp_heur[:2]
            if len(top_my) > 1 and len(opp_cands) >= 1:
                best_perm = top_my[0]
                best_ev = -1e18
                for i, mp in enumerate(top_my):
                    mb = [board[idx] for idx in mp]
                    ev = 0.0
                    w_sum = 0.0
                    for j, op in enumerate(opp_cands):
                        w = 0.60 if j == 0 else 0.20
                        ob = [opp_base[idx] for idx in op]
                        pset = ApexOpponentParticles(
                            opponent_player=particles.opponent_player,
                            tier=particles.tier,
                            boards=[ob],
                            highroll_board=ob,
                        )
                        ev += w * self._combat_score(
                            tavern, mb, pset, ctx, combats_per_particle=24, seed_offset=901 + i * 19 + j
                        )
                        w_sum += w
                    ev /= max(1e-9, w_sum)
                    if ev > best_ev:
                        best_ev = ev
                        best_perm = mp
                return best_perm

        return scored[0][1]

    @staticmethod
    def permutation_to_actions(perm: Sequence[int]) -> List[seaglass.TavernAction]:
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
