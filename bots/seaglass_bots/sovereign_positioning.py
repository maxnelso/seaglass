"""Sovereign End-of-Turn & In-Search Board Positioning Optimizer (`SovereignPositioner`)."""

from __future__ import annotations

import itertools
from typing import TYPE_CHECKING, Dict, List, Optional, Sequence, Tuple

import seaglass

from .apex_knowledge import ApexKnowledge, get_apex_knowledge
from .sovereign_belief import SovereignOpponentParticles

if TYPE_CHECKING:
    from .sovereign_evaluator import SovereignEvaluator


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


class SovereignPositioner:
    """
    Sovereign Board Positioning Optimizer:
      1. `<0.05ms` `fast_reposition_game` (steepest-ascent 2-opt + relocation local search) called
         INSIDE `SovereignEvaluator.evaluate_turn_end` so shop search accurately values `Young Murk-Eye`,
         `Persistent Poet`, `Stitched Salvager`, `Al'Akir`, `Sin'dorei`, Cleaves, and `Titus`/`Slamma`!
      2. CRN-paired Successive Halving Monte Carlo Combat Rollouts at End-of-Turn (`best_permutation`).
      3. Anti-Cleave splash defense (`particles.has_cleave_front`) + fresh-sighting (`seen_dt <= 1`)
         and Top-2 1v1 counter-positioning.
    """

    def __init__(
        self,
        catalog_name: str = "full",
        max_candidates: int = 56,
        evaluator: Optional["SovereignEvaluator"] = None,
    ) -> None:
        self.kb: ApexKnowledge = get_apex_knowledge(catalog_name)
        self.meta = self.kb.meta
        self.evaluator: Optional["SovereignEvaluator"] = evaluator
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
            bc_bonus = 15.0 + 0.65 * eng_map.get(cid, 4.0) if is_nontarget_bc else 0.0
            is_dragon_nb = ("dragon" in u.tribes or "all" in u.tribes) and cid != poet_id
            dragon_bonus = (13.5 + (4.5 if not u.divine_shield else 1.5)) if is_dragon_nb else 0.0
            has_dr = prof.has_deathrattle if prof else False
            salv_victim_val = (
                -22.0
                if (cid == salv_id or is_fragile)
                else ((13.0 if has_dr else 4.5) + 0.12 * min(120, u.attack + u.health))
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
        perm: Sequence[int],
        feats: List[Tuple],
        is_alakir: bool,
        is_illidan: bool,
        enemy_ds: bool,
        enemy_giant: bool,
        enemy_cleave: bool = False,
    ) -> float:
        n = len(perm)
        if n <= 1:
            return 0.0
        denom = float(n - 1)
        score = 0.0
        lev_pos = -1
        slamma_pos = -1
        splash_pen = 8.5 if enemy_cleave else 5.0
        taunt_adj_pen = 4.0 if enemy_cleave else 1.5

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
                _is_splash_vuln,
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
                    score += feats[perm[i - 1]][13] * (2.0 if is_gold else 1.0)
                if i + 1 < n:
                    score += feats[perm[i + 1]][13] * (2.0 if is_gold else 1.0)

            if is_poet:
                if i > 0:
                    score += feats[perm[i - 1]][14]
                if i + 1 < n:
                    score += feats[perm[i + 1]][14]

            if is_salv:
                if i == 0:
                    score -= 20.0
                else:
                    score += feats[perm[i - 1]][15]
                    if is_gold and i + 1 < n:
                        score += feats[perm[i + 1]][15]

            if i == 0 and is_alakir:
                if is_cleave or (atk >= 8 and not ds):
                    score += 12.0
                elif is_backline:
                    score -= 13.0
            if (i == 0 or i == n - 1) and is_illidan:
                if is_cleave or is_rally or atk >= 10:
                    score += 7.5

            if is_sin:
                score += 9.0 * (1.0 - frac)
            elif is_cleave or is_rally:
                score += 5.5 * (1.0 - frac)
            if wf and atk >= 8:
                score += 2.2 * (1.0 - frac)

            if is_scam:
                if enemy_ds and not ds and i == 0 and n >= 3:
                    score -= 5.0
                elif enemy_giant and 1 <= i <= 3:
                    score += 4.0

            if is_backline and not taunt and not is_murk:
                score += 6.0 * frac

            if is_summoner and not is_backline:
                if n == 7 and i == 0 and hp > 6:
                    score -= 1.2
                score += 2.6 * (1.0 - abs(frac - 0.32))

            if taunt:
                if i == 0 or i == n - 1:
                    score += 2.2
                if i > 0:
                    nb_l = feats[perm[i - 1]]
                    if nb_l[4]:
                        score -= taunt_adj_pen
                    if nb_l[12]:
                        score -= splash_pen
                    elif nb_l[7] and not nb_l[5]:
                        score -= 0.7 * splash_pen
                if i + 1 < n:
                    nb_r = feats[perm[i + 1]]
                    if nb_r[12]:
                        score -= splash_pen
                    elif nb_r[7] and not nb_r[5]:
                        score -= 0.7 * splash_pen

        if lev_pos >= 0 and slamma_pos >= 0:
            score += 9.0 if lev_pos < slamma_pos else -6.5

        return score

    @staticmethod
    def _constructive_seed_perm(feats: List[Tuple]) -> List[int]:
        n = len(feats)

        def _bucket(idx: int) -> Tuple[int, int]:
            f = feats[idx]
            stats = f[1] + f[2]
            if f[21]:  # Sin'dorei
                return (0, -stats - 500)
            if f[8] or f[9]:  # Cleave / Rally
                return (0, -stats)
            if f[10] and not f[11]:  # Summoner
                return (1, -stats)
            if not f[11] and not f[4]:  # Standard attacker
                return (2, -stats)
            if f[4] and not f[11]:  # Edge taunt near back/end
                return (4, -stats)
            return (3, stats)  # Backline

        return sorted(range(n), key=_bucket)

    def fast_best_perm(
        self,
        board: Sequence[seaglass.Unit],
        obs: Optional[seaglass.TavernObservation] = None,
        particles: Optional[SovereignOpponentParticles] = None,
    ) -> Tuple[int, ...]:
        """
        `<0.05ms` layout optimizer for in-search `evaluate_turn_end` physics rollouts!
        Uses exact enumeration for `n <= 4` (`<= 24` perms) and 2-start steepest-ascent
        swap + single-relocation local search for `n in {5, 6, 7}` (~110 evals vs 5,040!).
        """
        n = len(board)
        if n <= 1:
            return tuple(range(n))

        feats = self._precompute_unit_features(board)
        hero_name = self.kb.hero_names.get(obs.hero_id, "") if obs is not None else ""
        is_alakir = hero_name == "Al'Akir"
        is_illidan = hero_name == "Illidan Stormrage"
        enemy_ds = particles.has_divine_shields if particles is not None else False
        enemy_giant = particles.has_giant_unit if particles is not None else False
        enemy_cleave = particles.has_cleave_front if particles is not None else False

        if n <= 4:
            best_p = tuple(range(n))
            best_s = self._score_perm_from_feats(
                best_p, feats, is_alakir, is_illidan, enemy_ds, enemy_giant, enemy_cleave
            )
            for p in itertools.permutations(range(n)):
                s = self._score_perm_from_feats(
                    p, feats, is_alakir, is_illidan, enemy_ds, enemy_giant, enemy_cleave
                )
                if s > best_s + 1e-6:
                    best_s = s
                    best_p = p
            return best_p

        identity = list(range(n))
        seed_perm = self._constructive_seed_perm(feats)
        starts = [identity] if seed_perm == identity else [identity, seed_perm]

        global_best_p = tuple(identity)
        global_best_s = self._score_perm_from_feats(
            global_best_p, feats, is_alakir, is_illidan, enemy_ds, enemy_giant, enemy_cleave
        )

        for start in starts:
            cur = list(start)
            cur_s = self._score_perm_from_feats(
                cur, feats, is_alakir, is_illidan, enemy_ds, enemy_giant, enemy_cleave
            )
            for _ in range(4):
                best_next: Optional[List[int]] = None
                best_ns = cur_s + 1e-5
                # 1. Pairwise swaps (21 for n=7)
                for i in range(n - 1):
                    for j in range(i + 1, n):
                        cur[i], cur[j] = cur[j], cur[i]
                        ns = self._score_perm_from_feats(
                            cur, feats, is_alakir, is_illidan, enemy_ds, enemy_giant, enemy_cleave
                        )
                        if ns > best_ns:
                            best_ns = ns
                            best_next = list(cur)
                        cur[i], cur[j] = cur[j], cur[i]
                # 2. Single-unit relocation (important for inserting Murk-Eye/Poet/Salvager between pairs!)
                for src in range(n):
                    val = cur[src]
                    rem = cur[:src] + cur[src + 1 :]
                    for dst in range(n):
                        if dst == src or abs(dst - src) == 1:
                            continue
                        cand = rem[:dst] + [val] + rem[dst:]
                        ns = self._score_perm_from_feats(
                            cand, feats, is_alakir, is_illidan, enemy_ds, enemy_giant, enemy_cleave
                        )
                        if ns > best_ns:
                            best_ns = ns
                            best_next = cand
                if best_next is None:
                    break
                cur = best_next
                cur_s = best_ns

            if cur_s > global_best_s + 1e-5:
                global_best_s = cur_s
                global_best_p = tuple(cur)

        return global_best_p

    def fast_reposition_game(
        self,
        tavern: seaglass.TavernGame,
        obs: seaglass.TavernObservation,
        particles: Optional[SovereignOpponentParticles] = None,
    ) -> None:
        """Apply `<0.05ms` optimal board layout in-place on a `TavernGame` copy before `end_turn()`."""
        n = len(obs.board)
        if n <= 1:
            return
        perm = self.fast_best_perm(obs.board, obs=obs, particles=particles)
        if perm == tuple(range(n)):
            return
        for act in self.permutation_to_actions(perm):
            if tavern.is_legal(act):
                tavern.step(act)

    def _candidate_permutations(
        self,
        board: Sequence[seaglass.Unit],
        obs: Optional[seaglass.TavernObservation] = None,
        particles: Optional[SovereignOpponentParticles] = None,
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
        enemy_cleave = particles.has_cleave_front if particles is not None else False

        scored = sorted(
            unique_perms,
            key=lambda p: self._score_perm_from_feats(
                p, feats, is_alakir, is_illidan, enemy_ds, enemy_giant, enemy_cleave
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
        particles: SovereignOpponentParticles,
        ctx: Optional[seaglass.LobbyContext],
        combats_per_particle: int,
        seed_offset: int,
    ) -> float:
        assert self.evaluator is not None
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
                + 3.2 * dist.a_win_rate
                + 0.24 * dist.damage_mean
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
        particles: SovereignOpponentParticles,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> Tuple[int, ...]:
        assert self.evaluator is not None
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
        enemy_cleave = particles.has_cleave_front

        rounds = [(0.28, 6), (0.35, 20), (1.0, 48)]
        survivors = candidates
        scored: List[Tuple[float, Tuple[int, ...]]] = []

        for r_idx, (keep_frac, n_sims) in enumerate(rounds):
            scored = []
            # CRN: identical `seed_offset` across all candidate permutations in round `r_idx`!
            crn_offset = 109 * (r_idx + 1)
            for perm in survivors:
                cand_board = [board[i] for i in perm]
                val = self._combat_score(
                    tavern,
                    cand_board,
                    particles,
                    ctx,
                    combats_per_particle=n_sims,
                    seed_offset=crn_offset,
                )
                prior_w = 0.18 if has_eot_adj else 2.5e-3
                val += prior_w * self._score_perm_from_feats(
                    perm, feats, is_alakir, is_illidan, enemy_ds, enemy_giant, enemy_cleave
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
            for perm in pool_perms[:8]:
                sim = tavern.copy()
                for act in self.permutation_to_actions(perm):
                    if sim.is_legal(act):
                        sim.step(act)
                full_v = self.evaluator.evaluate_turn_end(
                    sim,
                    particles,
                    ctx=ctx,
                    combats_per_particle=20,
                    pre_positioned=True,
                )
                phys_scored.append((full_v, perm))
            phys_scored.sort(key=lambda x: x[0], reverse=True)
            scored = phys_scored

        # Counter-position whenever in Top-2 1v1 OR we have a fresh (`seen_dt <= 1`) opponent board!
        fresh_sighting = particles.seen_dt is not None and particles.seen_dt <= 1
        if (
            ctx is not None
            and (ctx.alive_units <= 3 or fresh_sighting)
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
                w0 = 0.68 if fresh_sighting else 0.56
                w_alt = (1.0 - w0) / max(1, len(opp_cands) - 1)
                for mp in top_my:
                    mb = [board[idx] for idx in mp]
                    ev = 0.0
                    w_sum = 0.0
                    for j, op in enumerate(opp_cands):
                        w = w0 if j == 0 else w_alt
                        ob = [opp_base[idx] for idx in op]
                        pset = SovereignOpponentParticles(
                            opponent_player=particles.opponent_player,
                            tier=particles.tier,
                            boards=[ob],
                            highroll_board=ob,
                        )
                        # CRN: same seed offset `901 + j * 29` across all `mp`!
                        ev += w * self._combat_score(
                            tavern, mb, pset, ctx, combats_per_particle=28, seed_offset=901 + j * 29
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
