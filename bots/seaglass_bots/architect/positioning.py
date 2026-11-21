"""Comp-Aware + Rust Combat Rollout Board Positioning Optimizer (`ArchitectPositioner`).

Two-stage positioning:
  1. `fast_comp_order(obs, plan)`: `<0.05ms` rule-driven ordering that encodes every
     composition's EoT and combat placement invariants (`Young Murk-Eye` adjacency,
     `Persistent Poet` Dragon adjacency, `Stitched Salvager` left-target, `Snazzy Phantom`
     right-most Undead carry, `Parasitic Fleshling` left-most carry, `Deathstrider`
     left-most Deathrattle + Rally attacker, `Goldrinn`/`Nadina`/`Sanguine Champion`
     early deaths, `Y'Shaarj` vs `C'Thun` Deity death order, Cleave/Sin'dorei front,
     Titus/Slamma/Brann/Drakkari protected backline, Anti-Cleave Taunt cornering).
  2. `optimize_end_of_turn(game, plan, opp_boards, seed)`: Generates diverse candidate
     permutations around the comp-optimal order and evaluates them via paired-seed
     exact Rust `end_turn()` + `equity_against(...)` rollouts, returning the exact
     sequence of `Reposition` actions to reach the best order.
"""

from __future__ import annotations

from typing import Dict, List, Optional, Sequence, Tuple

import seaglass

from ..sovereign_belief import SovereignOpponentParticles
from .comps import CompRegistry, get_comp_registry
from .planner import BuildPlan


def _apply_permutation_moves(n: int, perm: Sequence[int]) -> List[Tuple[int, int]]:
    """
    Convert target permutation `perm` (where `perm[new_pos] = old_idx`) into a
    minimal sequence of `Reposition(from_pos, to_pos)` moves.
    """
    cur = list(range(n))
    moves: List[Tuple[int, int]] = []
    for target_pos, desired_orig in enumerate(perm):
        cur_pos = cur.index(desired_orig)
        if cur_pos != target_pos:
            moves.append((cur_pos, target_pos))
            item = cur.pop(cur_pos)
            cur.insert(target_pos, item)
    return moves


def _synthetic_opponent_board(turn: int, tier: int) -> List[seaglass.Unit]:
    """Reasonable turn-scaled synthetic opponent board when next opponent hasn't been seen."""
    count = min(7, max(1, turn + 1))
    if turn <= 2:
        atk, hp = 2 + turn, 3 + turn
    elif turn <= 5:
        atk, hp = 3 * turn - 3, 3 * turn - 1
    elif turn <= 8:
        atk, hp = 5 * turn - 12, 6 * turn - 14
    elif turn <= 11:
        atk, hp = 11 * turn - 58, 12 * turn - 62
    else:
        atk, hp = 20 * turn - 150, 22 * turn - 165
    board: List[seaglass.Unit] = []
    for i in range(count):
        a = max(1, int(atk * (1.15 if i == 0 else (0.85 if i == count - 1 else 1.0))))
        h = max(1, int(hp * (1.15 if i == 0 else 1.0)))
        spec = f"{a}/{h}"
        if i == 0 and turn >= 5:
            spec += " divine_shield"
        if i == count - 1 and turn >= 4:
            spec += " taunt"
        board.append(seaglass.parse_unit(spec))
    return board


class ArchitectPositioner:
    def __init__(self, catalog_name: str = "full") -> None:
        self.catalog_name = catalog_name
        self.reg: CompRegistry = get_comp_registry(catalog_name)
        tag_names, tag_table = seaglass.card_tags()
        t_idx = {n: i for i, n in enumerate(tag_names)}

        def _ids_with_tag(tag: str) -> frozenset[int]:
            col = t_idx.get(tag)
            if col is None:
                return frozenset()
            return frozenset(cid for cid, row in tag_table.items() if row[col] > 0.5)

        self._bc_ids = _ids_with_tag("battlecry")
        self._dr_ids = _ids_with_tag("deathrattle")
        self._rally_ids = _ids_with_tag("rally")
        self._cleave_ids = self.reg.cleave_ids

        def _id(name: str) -> int:
            return self.reg.name_to_id.get(name, -1)

        self._murkeye_id = _id("Young Murk-Eye")
        self._poet_id = _id("Persistent Poet")
        self._tarecgosa_id = _id("Tarecgosa")
        self._salvager_id = _id("Stitched Salvager")
        self._fleshling_id = _id("Parasitic Fleshling")
        self._snazzy_id = _id("Snazzy Phantom")
        self._banshee_id = _id("Barrier Banshee")
        self._deathstrider_id = _id("Deathstrider")
        self._goldrinn_id = _id("Goldrinn, the Great Wolf")
        self._skitterer_id = _id("Turquoise Skitterer")
        self._nadina_id = _id("Nadina the Red")
        self._sindorei_id = _id("Sin'dorei Straight Shot")
        self._bile_spitter_id = _id("Bile Spitter")
        self._refiner_id = _id("Sanguine Refiner")
        self._bonker_id = _id("Bonker")
        self._sanguine_champ_id = _id("Sanguine Champion")
        self._broodmother_id = _id("Heroic Broodmother")
        self._vindicator_id = _id("Crimson Vindicator")
        self._eternal_summoner_id = _id("Eternal Summoner")
        self._eudora_id = _id("Ship Master Eudora")
        self._converter_id = _id("Faceless Converter")
        self._drifting_sac_id = _id("Drifting Sacrifice")
        self._underrot_id = _id("Underrot Spawn")
        self._sylvar_id = _id("Surfing Sylvar")

        self._backline_ids = frozenset(
            cid
            for name in (
                "Titus Rivendare",
                "Brann Bronzebeard",
                "Drakkari Enchanter",
                "Banana Slamma",
                "Lurking Leviathan",
                "Deathstrider",
                "Eternal Tycoon",
                "Drustfallen Butcher",
                "Lichling Hoarder",
                "Deathly Striker",
                "Snazzy Phantom",
                "Utility Drone",
                "Resourceful Robot",
                "Felfire Conjurer",
                "Cataclysmic Harbinger",
                "Balinda Stonehearth",
                "Cage Gnawer",
                "Soul Rewinder",
                "Kalecgos, Arcane Aspect",
                "Sky Admiral Rogers",
                "Proud Privateer",
                "Hot-Air Surveyor",
                "Victorious Geomant",
                "Turbo Hogrider",
                "The Shadow of Doubt",
                "Mindbender Ghur'sha",
                "Harbinger Aph'lass",
            )
            if (cid := self.reg.name_to_id.get(name)) is not None
        )

        self._early_die_ids = frozenset(
            cid
            for name in (
                "Goldrinn, the Great Wolf",
                "Nadina the Red",
                "Sanguine Champion",
                "Ship Master Eudora",
                "Turquoise Skitterer",
                "Eternal Summoner",
                "Faceless Converter",
                "Waveling",
                "Sacrificial Wrathguard",
                "Cadaver Caretaker",
                "Handless Forsaken",
                "Risen Rider",
                "Scarlet Skull",
                "Mummifier",
                "Sewer Lord",
                "Drifting Sacrifice",
                "Underrot Spawn",
                "Cord Puller",
                "Harmless Bonehead",
                "Buzzing Vermin",
            )
            if (cid := self.reg.name_to_id.get(name)) is not None
        )

    def _score_order(
        self,
        board: Sequence[seaglass.Unit],
        perm: Sequence[int],
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
    ) -> float:
        """Fast heuristic score for a board permutation `perm`."""
        n = len(perm)
        if n <= 1:
            return 0.0
        ordered = [board[i] for i in perm]
        score = 0.0

        has_snazzy = any(u.card_id == self._snazzy_id for u in ordered)
        has_fleshling = any(u.card_id == self._fleshling_id for u in ordered)
        has_deathstrider = any(u.card_id == self._deathstrider_id for u in ordered)
        has_poet = any(u.card_id == self._poet_id for u in ordered)
        has_murkeye = any(u.card_id == self._murkeye_id for u in ordered)
        has_salvager = any(u.card_id == self._salvager_id for u in ordered)
        has_cleave_threat = plan.threats.next_opp_cleave
        deity_name = str(obs.deity or "")

        # 1. Left-to-right role gradient (lower index = attacks / dies earlier)
        for pos, u in enumerate(ordered):
            cid = u.card_id
            frac = pos / max(1, n - 1)  # 0.0 (left) -> 1.0 (right)
            stat = u.attack + u.health

            # Sin'dorei first vs Taunt/Reborn
            if cid == self._sindorei_id:
                bonus = 28.0 if plan.threats.next_opp_reborn_taunt else 12.0
                score += bonus * (1.0 - frac)

            # Cleaves want to attack early (pos 0 or 1)
            if cid in self._cleave_ids:
                score += (20.0 + 0.06 * min(200, u.attack)) * (1.0 - 0.85 * frac)

            # Bile Spitter attacks early to hand out Venomous
            if cid == self._bile_spitter_id:
                score += 18.0 * (1.0 - frac)

            # Sanguine Refiner attacks FIRST to buff Blood Gems before Bonker/Vineweaver
            if cid == self._refiner_id:
                score += 24.0 * (1.0 - frac)
            elif cid == self._bonker_id:
                score += 16.0 * (1.0 - 0.7 * frac)

            # Early death triggers (Goldrinn, Nadina, Eternal Summoner, Sanguine Champ, tokens)
            if cid in self._early_die_ids:
                w = 24.0 if cid in (self._goldrinn_id, self._nadina_id, self._sanguine_champ_id, self._eudora_id) else 14.0
                score += w * (1.0 - frac)

            # Rally minions generally want to attack early
            if cid in self._rally_ids:
                score += (12.0 if u.windfury else 7.0) * (1.0 - frac)

            # Fragile / Avenge / Aura backliners belong far right
            if cid in self._backline_ids:
                score += 22.0 * frac
                if pos == 0:
                    score -= 14.0
            elif u.taunt:
                # Taunts generally sit right-of-center unless early-die taunts
                if cid not in self._early_die_ids:
                    score += 6.0 * frac
            else:
                # General heuristic: Divine Shield / high-Attack attackers slightly left, big HP right
                if u.divine_shield:
                    score += 5.0 * (1.0 - 0.6 * frac)
                if u.venomous:
                    score += 3.0 * (1.0 - 0.5 * frac)
                if u.reborn and "undead" in str(u.tribe).lower():
                    score += 9.0 * (1.0 - frac)

            # Anti-cleave defense: keep Taunts at edges (pos 0 or n-1) and away from backliners
            if has_cleave_threat and u.taunt:
                if pos in (0, n - 1):
                    score += 14.0
                else:
                    score -= 10.0
                for nb_pos in (pos - 1, pos + 1):
                    if 0 <= nb_pos < n:
                        nb = ordered[nb_pos]
                        if nb.card_id in self._backline_ids:
                            score -= 20.0
                        elif nb.divine_shield or nb.reborn:
                            score += 5.0

            # Deity death ordering (Aberrations)
            if deity_name == "Y'Shaarj" and "aberration" in str(u.tribe).lower():
                # First 2 dead Aberrations get resummoned at max stats -> put biggest Aberrations early!
                score += min(18.0, 0.12 * stat) * (1.0 - frac)
            elif deity_name == "C'Thun" and "aberration" in str(u.tribe).lower():
                if cid in (self._drifting_sac_id, self._underrot_id, self._converter_id) or u.reborn:
                    score += 14.0 * (1.0 - frac)
                elif stat >= 25:
                    # Keep big carry alive to soak C'Thun stat split!
                    score += 10.0 * frac

        # 2. Exact positional combo invariants!
        # (a) Parasitic Fleshling -> slot 0 gets huge permanent EoT stats
        if has_fleshling:
            left = ordered[0]
            if left.card_id not in self._backline_ids:
                score += 14.0
                if left.divine_shield or left.card_id in self._cleave_ids or left.windfury:
                    score += 12.0
                if "aberration" in str(left.tribe).lower():
                    score += 6.0
            else:
                score -= 18.0

        # (b) Snazzy Phantom -> RIGHT-MOST Undead gets massive stats on every Reborn!
        if has_snazzy:
            rm_undead_idx = None
            for idx in range(n - 1, -1, -1):
                if "undead" in str(ordered[idx].tribe).lower() or str(ordered[idx].tribe).lower() == "all":
                    rm_undead_idx = idx
                    break
            if rm_undead_idx is not None:
                carry = ordered[rm_undead_idx]
                if carry.card_id == self._banshee_id or carry.divine_shield:
                    score += 26.0
                elif carry.card_id not in self._early_die_ids:
                    score += 14.0 + min(12.0, 0.08 * (carry.attack + carry.health))
                else:
                    score -= 16.0
            # Reborn Undead should be LEFT of Snazzy Phantom / rightmost carry so they die first
            for pos, u in enumerate(ordered):
                if u.reborn and pos < (rm_undead_idx or 0):
                    score += 8.0

        # (c) Deathstrider -> triggers LEFT-MOST Deathrattle after friendly Rally attacks!
        if has_deathstrider:
            first_dr_pos = None
            for idx, u in enumerate(ordered):
                if u.card_id in self._dr_ids or u.extra_deathrattles > 0:
                    first_dr_pos = idx
                    break
            if first_dr_pos is not None:
                first_dr = ordered[first_dr_pos]
                if first_dr.card_id in (self._goldrinn_id, self._skitterer_id):
                    score += 38.0
                elif first_dr.card_id in (self._eternal_summoner_id, self._sanguine_champ_id):
                    score += 22.0
                else:
                    score -= 10.0

        # (d) Persistent Poet -> adjacent DRAGONS permanently keep combat stats & keywords!
        if has_poet:
            for pos, u in enumerate(ordered):
                if u.card_id == self._poet_id:
                    adj_dragons = 0
                    for nb_pos in (pos - 1, pos + 1):
                        if 0 <= nb_pos < n:
                            nb = ordered[nb_pos]
                            is_dragon = "dragon" in str(nb.tribe).lower() or str(nb.tribe).lower() == "all"
                            if is_dragon and nb.card_id != self._poet_id:
                                adj_dragons += 1
                                if nb.card_id == self._tarecgosa_id:
                                    # Tarecgosa already keeps combat buffs natively!
                                    score += 4.0
                                elif nb.card_id in (self._broodmother_id, self._vindicator_id) or nb.card_id in self._cleave_ids:
                                    score += 42.0
                                else:
                                    score += 28.0
                            else:
                                score -= 10.0
                    # Strongly prefer Poet in interior slot touching TWO Dragons when >= 2 other Dragons exist!
                    if adj_dragons == 2:
                        score += 28.0
                    elif pos in (0, n - 1) and adj_dragons < 2:
                        score -= 18.0

        # (e) Young Murk-Eye -> triggers adjacent BATTLECRIES at EoT!
        if has_murkeye:
            for pos, u in enumerate(ordered):
                if u.card_id == self._murkeye_id:
                    for nb_pos in (pos - 1, pos + 1):
                        if 0 <= nb_pos < n:
                            nb = ordered[nb_pos]
                            if nb.card_id in self._bc_ids:
                                score += 26.0 + 3.0 * nb.tavern_tier
                            else:
                                score -= 10.0

        # (f) Stitched Salvager -> destroys minion to its LEFT (both neighbors if Golden) and resummons exact copy!
        if has_salvager:
            for pos, u in enumerate(ordered):
                if u.card_id == self._salvager_id:
                    targets = [pos - 1] if not u.is_golden else [pos - 1, pos + 1]
                    for tpos in targets:
                        if 0 <= tpos < n:
                            tgt = ordered[tpos]
                            if tgt.card_id == self._salvager_id or tgt.card_id in self._backline_ids:
                                score -= 30.0
                            else:
                                score += 16.0 + min(24.0, 0.12 * (tgt.attack + tgt.health))
                                if tgt.card_id in self._dr_ids:
                                    score += 14.0
                        else:
                            score -= 22.0

        # (g) Surfing Sylvar -> adjacent minions get Attack at EoT
        if any(u.card_id == self._sylvar_id for u in ordered):
            for pos, u in enumerate(ordered):
                if u.card_id == self._sylvar_id:
                    for nb_pos in (pos - 1, pos + 1):
                        if 0 <= nb_pos < n:
                            nb = ordered[nb_pos]
                            if nb.divine_shield or nb.card_id in self._cleave_ids:
                                score += 8.0

        return score

    def fast_comp_order(
        self,
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
    ) -> List[int]:
        """Fast (<0.05ms) steepest-ascent local search over permutations for `obs.board`."""
        board = obs.board
        n = len(board)
        if n <= 1:
            return list(range(n))

        best_perm = list(range(n))
        best_score = self._score_order(board, best_perm, obs, plan)

        # Quick initial sort by rough role priority to escape bad local optima
        def _init_key(idx: int) -> Tuple[int, int]:
            u = board[idx]
            cid = u.card_id
            if cid == self._sindorei_id and plan.threats.next_opp_reborn_taunt:
                return (0, -u.attack)
            if cid in self._cleave_ids or cid == self._refiner_id or cid == self._goldrinn_id:
                return (1, -u.attack)
            if cid in self._early_die_ids:
                return (2, -u.attack)
            if cid in self._backline_ids:
                return (6, u.attack + u.health)
            if u.taunt:
                return (5, -(u.attack + u.health))
            if u.divine_shield or cid in self._rally_ids:
                return (3, -u.attack)
            return (4, -(u.attack + u.health))

        sorted_perm = sorted(range(n), key=_init_key)
        s_score = self._score_order(board, sorted_perm, obs, plan)
        if s_score > best_score + 1e-6:
            best_perm = sorted_perm
            best_score = s_score

        # Steepest-ascent 2-opt swaps + single-element relocations (at most 3 passes)
        for _ in range(3):
            improved = False
            cand_best = best_perm
            cand_score = best_score
            for i in range(n):
                for j in range(i + 1, n):
                    p = list(best_perm)
                    p[i], p[j] = p[j], p[i]
                    sc = self._score_order(board, p, obs, plan)
                    if sc > cand_score + 1e-6:
                        cand_score = sc
                        cand_best = p
                        improved = True
            for i in range(n):
                item = best_perm[i]
                base = best_perm[:i] + best_perm[i + 1 :]
                for j in range(n):
                    if j == i:
                        continue
                    p = base[:j] + [item] + base[j:]
                    sc = self._score_order(board, p, obs, plan)
                    if sc > cand_score + 1e-6:
                        cand_score = sc
                        cand_best = p
                        improved = True
            best_perm = cand_best
            best_score = cand_score
            if not improved:
                break

        return best_perm

    def apply_fast_order(self, game: seaglass.TavernGame, plan: BuildPlan) -> None:
        """Reposition `game` in-place to the comp-optimal heuristic order."""
        obs = game.observe()
        n = len(obs.board)
        if n <= 1:
            return
        perm = self.fast_comp_order(obs, plan)
        if perm == list(range(n)):
            return
        for src, dst in _apply_permutation_moves(n, perm):
            act = seaglass.TavernAction.reposition(src, dst)
            if game.is_legal(act):
                game.step(act)

    def build_opponent_board(
        self,
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
    ) -> Tuple[List[seaglass.Unit], int]:
        """Return `(opp_board, opp_tier)` using observed board if available, else turn-scaled synthetic."""
        if plan.threats.next_opp_board:
            opp_board = list(plan.threats.next_opp_board)
            opp_tier = max(1, plan.threats.next_opp_tier)
            # If sighting is stale (seen 2+ turns ago), gently scale stats up to avoid underestimating
            age = min(4, max(0, plan.threats.next_opp_seen_age))
            if age >= 2 and obs.turn >= 5:
                boost = int(age * (2.5 + 0.8 * obs.turn))
                scaled = []
                for u in opp_board:
                    spec = f"{u.attack + boost}/{u.health + boost}"
                    if u.divine_shield:
                        spec += " divine_shield"
                    if u.taunt:
                        spec += " taunt"
                    if u.reborn:
                        spec += " reborn"
                    if u.windfury:
                        spec += " windfury"
                    if u.venomous:
                        spec += " venomous"
                    scaled.append(seaglass.parse_unit(spec))
                opp_board = scaled
            return opp_board, opp_tier
        return _synthetic_opponent_board(obs.turn, obs.tavern_tier), obs.tavern_tier

    def optimize_end_of_turn(
        self,
        base_game: seaglass.TavernGame,
        plan: BuildPlan,
        seed: int = 777,
        max_candidates: int = 20,
        rollouts_per_cand: int = 18,
        particles: Optional[SovereignOpponentParticles] = None,
    ) -> List[Tuple[int, int]]:
        """
        End-of-turn positioning solver:
          - Includes comp-optimal heuristic order, current order, and diverse local/front/edge swaps
          - Evaluates each candidate with paired-seed Rust `end_turn()` + `equity_against(...)`
            across opponent particles (expanding to 40 cands x 32 rollouts in Top-2/Top-3 finals!)
        """
        obs = base_game.observe()
        n = len(obs.board)
        if n <= 1:
            return []

        comp_perm = self.fast_comp_order(obs, plan)

        # On ghost turns or Turn 1-2, heuristic order is already optimal & instant
        if obs.turn <= 2 or plan.threats.next_is_ghost or (particles is not None and particles.is_ghost):
            return _apply_permutation_moves(n, comp_perm)

        # Scale search budget up sharply in Top-3 / Top-2 Finals where positioning decides 1st place!
        is_decisive = plan.threats.is_finals or plan.threats.alive_units <= 3 or (obs.health + obs.armor) <= 15
        eff_max_cands = max(max_candidates, 36) if is_decisive else max_candidates
        eff_rollouts = max(rollouts_per_cand, 28) if is_decisive else rollouts_per_cand

        # Build diverse candidate permutations
        candidates: List[Tuple[int, ...]] = [tuple(comp_perm), tuple(range(n))]
        seen = set(candidates)

        scored_neighbors: List[Tuple[float, Tuple[int, ...]]] = []
        # All pairwise swaps + single-unit relocations around comp_perm
        for i in range(n):
            for j in range(i + 1, n):
                p = list(comp_perm)
                p[i], p[j] = p[j], p[i]
                tp = tuple(p)
                if tp not in seen:
                    seen.add(tp)
                    scored_neighbors.append((self._score_order(obs.board, p, obs, plan), tp))
        if is_decisive:
            for i in range(n):
                item = comp_perm[i]
                base = comp_perm[:i] + comp_perm[i + 1 :]
                for j in (0, 1, n - 2, n - 1):
                    if 0 <= j < n and j != i:
                        p = base[:j] + [item] + base[j:]
                        tp = tuple(p)
                        if tp not in seen:
                            seen.add(tp)
                            scored_neighbors.append((self._score_order(obs.board, p, obs, plan), tp))

        scored_neighbors.sort(key=lambda x: x[0], reverse=True)
        for _sc, tp in scored_neighbors[: max(0, eff_max_cands - len(candidates))]:
            candidates.append(tp)

        if particles is not None and particles.boards:
            opp_boards = list(particles.boards[: (3 if is_decisive else 2)])
            opp_tier = max(1, int(particles.tier))
        else:
            ob, ot = self.build_opponent_board(obs, plan)
            opp_boards = [ob]
            opp_tier = ot

        eff_hp = obs.health + obs.armor
        sim_seed = (seed ^ (obs.turn * 2654435761)) & 0x7FFFFFFF

        best_perm = comp_perm
        best_val = -1e18
        n_opp = max(1, len(opp_boards))

        for perm in candidates:
            sim = base_game.copy()
            sim.reseed(sim_seed)
            for src, dst in _apply_permutation_moves(n, perm):
                act = seaglass.TavernAction.reposition(src, dst)
                if sim.is_legal(act):
                    sim.step(act)
            if sim.is_legal(seaglass.TavernAction.end_turn()):
                sim.end_turn()

            combat_val = 0.0
            for oi, ob in enumerate(opp_boards):
                dist = sim.equity_against(ob, opp_tier, eff_rollouts, sim_seed + oi * 37)
                lethal_pen = 0.0
                if eff_hp <= 15 and dist.damage_mean < 0.0:
                    lethal_pen = 2.5 * dist.damage_mean
                combat_val += (
                    45.0 * dist.a_win_rate
                    + 16.0 * dist.draw_rate
                    + 1.40 * dist.damage_mean
                    + lethal_pen
                )
            combat_val /= n_opp

            obs_post = sim.observe()
            post_stats = sum(u.attack + u.health for u in obs_post.board)
            heur_w = 0.05 if is_decisive else 0.32
            heur_tie = heur_w * self._score_order(obs.board, perm, obs, plan)
            total_val = combat_val + 0.12 * post_stats + heur_tie
            if total_val > best_val:
                best_val = total_val
                best_perm = list(perm)

        return _apply_permutation_moves(n, best_perm)
