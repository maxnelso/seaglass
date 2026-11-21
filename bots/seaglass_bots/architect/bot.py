"""ArchitectBot: Composition-Driven Macro-Guided Shop-Epoch Beam Search + Rust Physics Bot."""

from __future__ import annotations

import random
from dataclasses import dataclass
from typing import Dict, FrozenSet, List, Optional, Sequence, Tuple

import seaglass

from ..sovereign_belief import SovereignBeliefTracker, SovereignOpponentParticles
from .comps import CompRegistry, get_comp_registry
from .evaluator import _HARBINGER_PAYOFF_SPELLS, ArchitectEvaluator
from .planner import BuildPlan, CompPlanner
from .positioning import ArchitectPositioner, _apply_permutation_moves

ActionKey = Tuple[str, Optional[int], Optional[int]]


def _action_key(a: seaglass.TavernAction) -> ActionKey:
    a0, a1 = a.args
    return (a.kind, a0, a1)


def _state_sig(obs: seaglass.TavernObservation) -> Tuple:
    return (
        obs.gold,
        obs.tavern_tier,
        obs.upgrade_cost,
        obs.is_frozen,
        obs.free_refreshes,
        obs.hero_power_uses_left,
        getattr(obs, "second_hero_power_uses_left", 0),
        getattr(obs, "dark_discoveries_left", 0),
        tuple((u.card_id, u.attack, u.health, u.is_golden, u.reborn, u.divine_shield) for u in obs.board),
        tuple((c.card_id, c.attack, c.health, c.is_golden, c.is_spell) for c in obs.hand),
        tuple((s.card_id, s.attack, s.health, s.is_spell) for s in obs.shop),
        tuple(obs.player_counters),
    )


def _validation_sig(obs: seaglass.TavernObservation) -> Tuple:
    return (
        obs.turn,
        obs.gold,
        obs.tavern_tier,
        obs.upgrade_cost,
        obs.free_refreshes,
        obs.discover_pending is not None,
        bool(obs.trinket_options),
        tuple((u.card_id, u.attack, u.health, u.is_golden) for u in obs.board),
        tuple((c.card_id, c.is_spell, c.is_golden) for c in obs.hand),
        tuple((s.card_id, s.is_spell) for s in obs.shop),
    )


@dataclass
class _BeamNode:
    game: seaglass.TavernGame
    obs: seaglass.TavernObservation
    actions: Tuple[seaglass.TavernAction, ...]
    fast_val: float
    # Whether this node already performed a stochastic step (stop expanding further!)
    stochastic_done: bool = False


class ArchitectBot:
    """
    Build-driven Battlegrounds bot:
      1. Analyzes the 5-tribe lobby & board/hand/trinkets/counters via `CompPlanner`
      2. Enforces build-specific Tavern sequencing macros (Undead Reborn right-most prep,
         Eat-the-Tavern buff-before-eat, Beatboxer/Drone Duplicator magnetization,
         Victorious Geomant hand/overflow prep, Harbinger payoff-spell-last, Fishbait
         Rally leftmost, Thorned Trailblazer best Choose-One first, Greedy Conniver T7)
      3. Runs a fast shop-epoch beam search scored by exact 1-turn Rust physics (`end_turn` + `equity_against`)
      4. Optimizes End-of-Turn board positioning via paired-seed Rust combat rollouts
    """

    def __init__(
        self,
        catalog_name: str = "full",
        seed: int = 42,
        beam_width: int = 18,
        max_depth: int = 10,
        leaf_eval_k: int = 10,
        leaf_rollouts: int = 14,
        pos_candidates: int = 22,
        pos_rollouts: int = 20,
        use_neural_value: bool = True,
        neural_weight: float = 0.22,
    ) -> None:
        self.catalog_name = catalog_name
        self.rng = random.Random(seed)
        self.seed = seed
        self.beam_width = beam_width
        self.max_depth = max_depth
        self.leaf_eval_k = leaf_eval_k
        self.leaf_rollouts = leaf_rollouts
        self.pos_candidates = pos_candidates
        self.pos_rollouts = pos_rollouts

        self.reg: CompRegistry = get_comp_registry(catalog_name)
        self.planner = CompPlanner(catalog_name)
        self.belief = SovereignBeliefTracker(catalog_name=catalog_name, seed=seed ^ 0x55AA55)
        self.evaluator = ArchitectEvaluator(
            catalog_name,
            use_neural_value=use_neural_value,
            neural_weight=neural_weight,
        )
        self.positioner = self.evaluator.positioner

        # Pre-bind special card IDs for Tavern sequencing macros
        def _id(name: str) -> int:
            return self.reg.name_to_id.get(name, -1)

        self._snazzy_id = _id("Snazzy Phantom")
        self._banshee_id = _id("Barrier Banshee")
        self._bellringer_id = _id("Dead Bellringer")
        self._beatboxer_id = _id("Polarizing Beatboxer")
        self._duplicator_id = _id("Drone Duplicator")
        self._geomant_id = _id("Victorious Geomant")
        self._surveyor_id = _id("Hot-Air Surveyor")
        self._roogug_id = _id("Geomagus Roogug")
        self._amalgam_id = _id("Gatekeeper Amalgam")
        self._harbinger_id = _id("Cataclysmic Harbinger")
        self._lionfish_id = _id("Lurking Lionfish")
        self._shark_id = _id("Snarky Shark")
        self._living_prison_id = _id("Living Prison")
        self._conniver_id = _id("Greedy Conniver")
        self._deft_deserter_id = _id("Deft Deserter")
        self._jailer_id = _id("Soulkeeping Jailer")
        self._trailblazer_id = _id("Thorned Trailblazer")
        self._glambot_id = _id("Glambot")
        self._tidehunter_id = _id("Twilight Tidehunter")
        self._distractor_id = _id("Devilish Distractor")
        self._rewinder_id = _id("Soul Rewinder")
        self._corruptor_id = _id("Ashen Corruptor")
        self._malchezaar_id = _id("Malchezaar, Prince of Dance")
        self._behemoth_id = _id("Arcane Behemoth")
        self._bream_id = _id("Bream Counter")
        self._scout_id = _id("Flighty Scout")
        self._choral_id = _id("Choral Mrrrglr")
        self._costume_id = _id("Costume Enthusiast")
        self._diremuck_id = _id("Diremuck Forager")
        self._futurefin_id = _id("Futurefin")
        self._sludge_names: FrozenSet[str] = frozenset({"Sludge Corrosion"})
        self._stochastic_spell_names: FrozenSet[str] = frozenset(
            {
                "Triple Reward",
                "A New Sprout",
                "Boundless Potential",
                "Contracted Corpse",
                "Hired Headhunter",
                "Planar Telescope",
                "Search Through Time",
                "Fandral's Fortune",
                "Hallowed Ritual",
                "Unmasked Identity",
                "Friendly Bounty",
                "Cloning Conch",
                "Chef's Choice",
                "Enchanted Lasso",
                "Recruit a Trainee",
                "Saloon's Finest",
                "Lost Staff of Hamuul",
                "Tomb Turning",
                "Time Management",
                "Forest's Bounty",
                "Alliance Flag",
            }
        )

        # Hook tags for fast macro filtering
        tag_names, tag_table = seaglass.card_tags()
        t_idx = {n: i for i, n in enumerate(tag_names)}

        def _ids_with_tag(tag: str) -> FrozenSet[int]:
            col = t_idx.get(tag)
            if col is None:
                return frozenset()
            return frozenset(cid for cid, row in tag_table.items() if row[col] > 0.5)

        self._bc_ids = _ids_with_tag("battlecry")
        self._bc_target_ids = _ids_with_tag("battlecry_target")
        self._rally_ids = _ids_with_tag("rally")
        self._choose_one_ids = _ids_with_tag("choose_one")
        self._sell_trigger_ids = _ids_with_tag("sell")

        # Per-turn queued plan: list of (action, expected_validation_sig_before)
        self._plan_queue: List[Tuple[seaglass.TavernAction, Optional[Tuple]]] = []
        self._last_turn: int = -1
        self._rolls_this_turn: int = 0
        self._last_plan: Optional[BuildPlan] = None
        self.last_note: Optional[Dict[str, float]] = None
        self.last_plan_summary: str = ""

    # ------------------------------------------------------------------
    # Public LobbyActor interface
    # ------------------------------------------------------------------
    def act_in_lobby(
        self,
        lobby: seaglass.LobbyGame,
        player: int,
    ) -> seaglass.TavernAction:
        legal = lobby.legal_actions(player)
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            self.last_note = None
            return legal[0]

        obs = lobby.observe(player)
        ctx = lobby.context(player)

        if obs.turn != self._last_turn:
            self._last_turn = obs.turn
            self._rolls_this_turn = 0
            self._plan_queue.clear()

        # 1. Hero Selection Phase
        if lobby.phase == "hero" or any(a.kind == "choose_hero" for a in legal):
            return self._choose_hero(obs, ctx, legal)

        # Refresh BuildPlan & multi-particle opponent beliefs on live state
        plan = self.planner.analyze(obs, ctx)
        self._last_plan = plan
        particles = self.belief.next_opponent_particles(
            obs.turn, player, ctx, lobby_id=id(lobby)
        )

        # 2. Trinket Selection Prompt (2-Turn CRN Physics Rollout + Comp Synergy!)
        trinket_acts = [a for a in legal if a.kind == "choose_trinket"]
        if trinket_acts:
            self._plan_queue.clear()
            return self._choose_trinket(lobby, player, obs, plan, trinket_acts, particles=particles, ctx=ctx)

        # 3. Discover / Choose-One / Dark-Discovery Prompt (Exact post-play Turn-End Physics!)
        discover_acts = [a for a in legal if a.kind == "choose_discover"]
        if discover_acts:
            self._plan_queue.clear()
            return self._choose_discover(lobby, player, obs, plan, discover_acts, particles=particles, ctx=ctx)

        # 4. Execute queued deterministic plan step if state still matches!
        if self._plan_queue:
            next_act, expected_sig = self._plan_queue[0]
            if (expected_sig is None or _validation_sig(obs) == expected_sig) and lobby.is_legal(
                player, next_act
            ):
                self._plan_queue.pop(0)
                if next_act.kind == "refresh":
                    self._rolls_this_turn += 1
                self.last_plan_summary = (
                    f"[Architect] {plan.primary_id}({plan.commitment:.2f}) T->{plan.target_tier} "
                    f"| queued={next_act.kind}{next_act.args}"
                )
                self.last_note = {
                    f"comp:{plan.primary_id}": round(plan.commitment, 3),
                    "target_tier": float(plan.target_tier),
                }
                return next_act
            self._plan_queue.clear()

        # 5. Plan current shop epoch via beam search + exact 1-turn Rust physics!
        base_game = lobby.belief_tavern(player, self.rng.getrandbits(63) | 1)
        chosen_seq = self._plan_shop_epoch(base_game, obs, plan, ctx=ctx, particles=particles)

        if not chosen_seq:
            return seaglass.TavernAction.end_turn()

        first = chosen_seq[0]
        if not lobby.is_legal(player, first):
            return seaglass.TavernAction.end_turn()

        # Queue deterministic tail steps with validation signatures so we don't re-search every click
        if len(chosen_seq) > 1 and first.kind not in (
            "refresh",
            "dark_discovery",
            "hero_power",
            "hero_power_2",
        ):
            sim = base_game.copy()
            sim.reseed(self.rng.getrandbits(63) | 1)
            if sim.is_legal(first):
                sim.step(first)
                for step_act in chosen_seq[1:]:
                    sim_obs = sim.observe()
                    if sim_obs.discover_pending is not None or sim_obs.trinket_options:
                        break
                    self._plan_queue.append((step_act, _validation_sig(sim_obs)))
                    if step_act.kind in ("refresh", "dark_discovery", "end_turn"):
                        break
                    if not sim.is_legal(step_act):
                        self._plan_queue.clear()
                        break
                    sim.step(step_act)

        if first.kind == "refresh":
            self._rolls_this_turn += 1

        self.last_plan_summary = (
            f"[Architect] {plan.primary_id}({plan.commitment:.2f}) T->{plan.target_tier} "
            f"| {first.kind}{first.args} (len={len(chosen_seq)})"
        )
        self.last_note = {
            f"comp:{plan.primary_id}": round(plan.commitment, 3),
            "target_tier": float(plan.target_tier),
        }
        return first

    # ------------------------------------------------------------------
    # Prompt Handlers: Hero, Trinket, Discover
    # ------------------------------------------------------------------
    def _choose_hero(
        self,
        obs: seaglass.TavernObservation,
        ctx: seaglass.LobbyContext,
        legal: Sequence[seaglass.TavernAction],
    ) -> seaglass.TavernAction:
        tribes = list(ctx.tribes) if ctx and ctx.tribes else []
        hero_opts = list(obs.hero_options)
        best_act = legal[0]
        best_sc = -1e18
        for a in legal:
            if a.kind != "choose_hero":
                continue
            idx = a.args[0]
            if idx is not None and idx < len(hero_opts):
                hid = hero_opts[idx]
                sc = self.evaluator.score_hero(hid, tribes)
                if sc > best_sc:
                    best_sc = sc
                    best_act = a
        self.last_plan_summary = f"[Architect] hero={best_act.args[0]} score={best_sc:.1f}"
        self.last_note = {"hero_score": round(best_sc, 2)}
        return best_act

    def _choose_trinket(
        self,
        lobby: seaglass.LobbyGame,
        player: int,
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
        trinket_acts: Sequence[seaglass.TavernAction],
        particles: Optional[SovereignOpponentParticles] = None,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> seaglass.TavernAction:
        """2-Turn CRN Physics Rollout + Comp-Synergy Trinket Selector!"""
        t_opts = list(obs.trinket_options)
        t_costs = list(obs.trinket_option_costs)
        base_game = lobby.belief_tavern(player, self.rng.getrandbits(63) | 1)
        opp_board, opp_tier = self.positioner.build_opponent_board(obs, plan)
        if particles is not None and particles.boards:
            opp_board = list(particles.boards[0])
            opp_tier = max(1, int(particles.tier))
        crn_seeds = (self.rng.getrandbits(63) | 1, self.rng.getrandbits(63) | 1)

        best_act = trinket_acts[0]
        best_sc = -1e18
        for a in trinket_acts:
            idx = a.args[0]
            if idx is None or idx >= len(t_opts):
                continue
            tid = t_opts[idx]
            cost = t_costs[idx] if idx < len(t_costs) else 0
            prior = self.evaluator.score_trinket(tid, cost, obs, plan)

            # 2-turn CRN rollout: pick trinket -> finish Turn t -> combat -> start Turn t+1 -> finish Turn t+1!
            roll_sum = 0.0
            for s_seed in crn_seeds:
                sim = base_game.copy()
                sim.reseed(s_seed)
                sim.step(a)
                # Re-analyze plan with new trinket owned!
                p_after = self.planner.analyze(sim.observe(), ctx)
                self._greedy_finish_epoch(sim, p_after, s_seed + 19, max_steps=6)
                v_t0 = self.evaluator.evaluate_turn_end(
                    sim,
                    p_after,
                    opp_board,
                    opp_tier,
                    seed=s_seed + 31,
                    rollouts=8,
                    ctx=ctx,
                    particles=particles,
                )
                # Advance 1 full turn to measure trinket snowball on Turn t+1!
                self.positioner.apply_fast_order(sim, p_after)
                if sim.is_legal(seaglass.TavernAction.end_turn()):
                    sim.end_turn()
                sim.reseed(s_seed + 53)
                sim.resolve_combat_against(opp_board, opponent_tier=opp_tier, seed=s_seed + 53)
                sim.reseed(s_seed + 71)
                sim.start_turn()
                p_t1 = self.planner.analyze(sim.observe(), ctx)
                self._greedy_finish_epoch(sim, p_t1, s_seed + 89, max_steps=6)
                v_t1 = self.evaluator.evaluate_fast(sim.observe(), p_t1, in_turn=False)
                roll_sum += 0.55 * v_t0 + 0.45 * v_t1

            sc = 0.80 * prior + (roll_sum / len(crn_seeds))
            if sc > best_sc:
                best_sc = sc
                best_act = a
        self.last_plan_summary = f"[Architect] {plan.primary_id} | trinket={best_act.args[0]} sc={best_sc:.1f}"
        self.last_note = {"trinket_score": round(best_sc, 2)}
        return best_act

    def _choose_discover(
        self,
        lobby: seaglass.LobbyGame,
        player: int,
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
        discover_acts: Sequence[seaglass.TavernAction],
        particles: Optional[SovereignOpponentParticles] = None,
        ctx: Optional[seaglass.LobbyContext] = None,
    ) -> seaglass.TavernAction:
        base_game = lobby.belief_tavern(player, self.rng.getrandbits(63) | 1)
        opp_board, opp_tier = self.positioner.build_opponent_board(obs, plan)
        sim_seed = self.rng.getrandbits(63) | 1
        best_act = discover_acts[0]
        best_sc = -1e18

        for a in discover_acts:
            sim = base_game.copy()
            sim.reseed(sim_seed)
            sim.step(a)
            o1 = sim.observe()

            # Re-analyze plan in case this discover IS a game-winning lynchpin!
            p1 = self.planner.analyze(o1, ctx)
            desire = 0.0
            dg_bonus = 0.0
            if len(o1.hand) > len(obs.hand):
                new_card = o1.hand[-1]
                desire = p1.card_desire(
                    new_card.card_id, new_card.name, new_card.tavern_tier, is_spell=new_card.is_spell
                )
                dg_bonus = sum(
                    0.85 * self.evaluator.kb.dark_gift_weights.get(dg, 4.5)
                    for dg in new_card.dark_gifts
                )

            # Greedily play the discovered card + spend remaining gold, then score with EXACT Turn-End Physics!
            self._greedy_finish_epoch(sim, p1, sim_seed + 17, max_steps=5)
            v_end = self.evaluator.evaluate_turn_end(
                sim,
                p1,
                opp_board,
                opp_tier,
                seed=sim_seed + 37,
                rollouts=8,
                ctx=ctx,
                particles=particles,
            )
            sc = v_end + 0.45 * desire + 0.50 * dg_bonus
            if sc > best_sc:
                best_sc = sc
                best_act = a

        self.last_plan_summary = f"[Architect] {plan.primary_id} | discover={best_act.args[0]} sc={best_sc:.1f}"
        self.last_note = {"discover_score": round(best_sc, 2)}
        return best_act

    # ------------------------------------------------------------------
    # Build-Specific Helper Filters & Macros
    # ------------------------------------------------------------------
    def _weakest_sell_slot(
        self,
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
        exclude_positions: FrozenSet[int] = frozenset(),
    ) -> Optional[int]:
        if not obs.board:
            return None
        best_idx = None
        best_prot = 1e18
        for i, u in enumerate(obs.board):
            if i in exclude_positions:
                continue
            prot = plan.sell_protection(u)
            # Slight tie-break toward weaker raw stats & lower tier
            key = prot + 0.04 * (u.attack + u.health) + 0.25 * u.tavern_tier
            if key < best_prot:
                best_prot = key
                best_idx = i
        return best_idx

    def _ranked_sell_slots(
        self,
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
        limit: int = 2,
    ) -> List[int]:
        scored = []
        for i, u in enumerate(obs.board):
            prot = plan.sell_protection(u) + 0.04 * (u.attack + u.health) + 0.25 * u.tavern_tier
            scored.append((prot, i))
        scored.sort(key=lambda x: x[0])
        return [i for _p, i in scored[:limit]]

    def _best_rightmost_undead_move(self, obs: seaglass.TavernObservation) -> Optional[seaglass.TavernAction]:
        """For `Snazzy Phantom`: move best Undead carry to the right-most board slot before Tavern Reborn!"""
        board = obs.board
        if len(board) <= 1 or not any(u.card_id == self._snazzy_id for u in board):
            return None
        best_idx = None
        best_sc = -1e18
        for i, u in enumerate(board):
            if not ("undead" in str(u.tribe).lower() or str(u.tribe).lower() == "all"):
                continue
            if u.card_id == self._bellringer_id:
                continue
            sc = (
                (60.0 if u.card_id == self._banshee_id else 0.0)
                + (30.0 if u.divine_shield else 0.0)
                + (18.0 if u.reborn else 0.0)
                + 0.4 * (u.attack + u.health)
            )
            if sc > best_sc:
                best_sc = sc
                best_idx = i
        if best_idx is not None and best_idx != len(board) - 1:
            return seaglass.TavernAction.reposition(best_idx, len(board) - 1)
        return None

    def _best_spell_target_positions(
        self,
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
        card: seaglass.Unit,
        legal_plays: Sequence[seaglass.TavernAction],
    ) -> List[seaglass.TavernAction]:
        """Prune targeted spell plays to the top 2–3 build-optimal board targets."""
        if len(legal_plays) <= 2:
            return list(legal_plays)
        board = obs.board
        cname = card.name
        has_glambot = any(u.card_id == self._glambot_id for u in board)
        scored: List[Tuple[float, seaglass.TavernAction]] = []

        for a in legal_plays:
            tpos = a.args[1]
            if tpos is None or tpos >= len(board):
                scored.append((0.0, a))
                continue
            u = board[tpos]
            sc = 0.0

            # Special-case spells!
            if cname == "Butchering":
                # Destroy a Reborn Undead or Plaguerunner so it triggers Tavern Reborn / 2x Atk!
                sc += 60.0 if u.reborn else 0.0
                sc += 55.0 if u.name == "Plaguerunner" else 0.0
                sc += 35.0 if u.name == "Eternal Summoner" else 0.0
                sc -= plan.sell_protection(u) * (0.15 if u.reborn else 1.2)
            elif cname == "Channel the Devourer":
                # Sell our weakest stats/non-protected or transfer huge temporary unit onto DS/cleave
                sc = (u.attack + u.health) * 0.4 - plan.sell_protection(u)
            elif cname == "Robust Evolution":
                # Transform a weak/deadwood minion of tier <= 5 into next tier
                sc = 10.0 * u.tavern_tier - plan.sell_protection(u)
            elif cname == "Eyes of the Earth Mother":
                # Golden Greedy Conniver (for Tier 7!) or a Tier 4 engine (`Persistent Poet`, `Flaming Enforcer`, `Geomagus Roogug`, `Blade Collector`)!
                if u.card_id == self._conniver_id:
                    sc = 120.0
                else:
                    sc = 2.0 * plan.card_desire(u.card_id, u.name, u.tavern_tier) + 12.0 * u.tavern_tier
            elif cname == "Perfect Vision":
                # Set a low-stat DS/Cleave/Windfury unit to 20/20!
                gain = max(0, 20 - u.attack) + max(0, 20 - u.health)
                sc = gain + (18.0 if u.divine_shield else 0.0) + (20.0 if u.card_id in self.reg.cleave_ids else 0.0)
            else:
                # Standard buff spell (Blood Gem, Banana, Fortify, Seafood Stew, Repair Job, Pointy Arrow, etc.)
                if u.card_id == self._amalgam_id:
                    sc += 85.0  # Gatekeeper Amalgam casts Misplaced Tea Set!
                if u.card_id == self._tidehunter_id:
                    sc += 65.0  # Twilight Tidehunter gives +9/+9 to leftmost in hand!
                if u.card_id == self._distractor_id:
                    sc += 55.0  # Devilish Distractor buffs shop +1/+2 rest of game!
                if u.card_id == self._roogug_id and cname == "Blood Gem":
                    sc += 70.0  # Roogug bounces every Blood Gem to another minion!
                if has_glambot and ("mech" in str(u.tribe).lower() or str(u.tribe).lower() == "all"):
                    sc += 48.0  # Glambot magnetizes a 5/5 Satellite onto any Mech!
                if u.card_id in self.reg.cleave_ids:
                    sc += 32.0
                if u.divine_shield:
                    sc += 22.0
                if u.windfury:
                    sc += 14.0
                sc += 0.15 * plan.sell_protection(u) + 0.08 * (u.attack + u.health)

            scored.append((sc, a))

        scored.sort(key=lambda x: x[0], reverse=True)
        return [a for _sc, a in scored[:2]]

    def _candidate_macros(
        self,
        game: seaglass.TavernGame,
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
    ) -> List[Tuple[Tuple[seaglass.TavernAction, ...], bool]]:
        """
        Generate pruned, build-aware candidate actions and atomic multi-step macros
        from `obs`. Returns list of `(action_tuple, is_stochastic)`.
        """
        legal = game.legal_actions()
        if not legal:
            return []

        board = obs.board
        hand = obs.hand
        shop = obs.shop
        n_board = len(board)

        out: List[Tuple[Tuple[seaglass.TavernAction, ...], bool]] = []
        has_beatboxer = any(u.card_id == self._beatboxer_id for u in board)
        has_harbinger = any(u.card_id == self._harbinger_id for u in board)
        has_surveyor = any(u.card_id == self._surveyor_id for u in board)
        has_rewinder = any(u.card_id in (self._rewinder_id, self._corruptor_id) for u in board)

        # Check if Harbinger is holding BOTH a payoff spell and a filler spell -> cast filler FIRST!
        harbinger_hold_payoff = False
        if has_harbinger:
            has_payoff_in_hand = any(c.is_spell and c.name in _HARBINGER_PAYOFF_SPELLS for c in hand)
            has_filler_in_hand = any(
                c.is_spell and not c.unplayable and c.name not in _HARBINGER_PAYOFF_SPELLS and c.name != "Blood Gem"
                for c in hand
            )
            harbinger_hold_payoff = has_payoff_in_hand and has_filler_in_hand

        # 1. Play actions from hand (grouped by hand_index)
        plays_by_hand: Dict[int, List[seaglass.TavernAction]] = {}
        for a in legal:
            if a.kind == "play" and a.args[0] is not None:
                plays_by_hand.setdefault(a.args[0], []).append(a)

        for h_idx, acts in plays_by_hand.items():
            if h_idx >= len(hand):
                continue
            card = hand[h_idx]
            if card.is_spell:
                if harbinger_hold_payoff and card.name in _HARBINGER_PAYOFF_SPELLS:
                    continue  # Cast cheap/filler spells before Harbinger payoff spell!
                is_stoch = card.name in (
                    "Triple Reward",
                    "A New Sprout",
                    "Boundless Potential",
                    "Contracted Corpse",
                    "Hired Headhunter",
                    "Planar Telescope",
                    "Search Through Time",
                    "Fandral's Fortune",
                    "Hallowed Ritual",
                    "Unmasked Identity",
                    "Friendly Bounty",
                    "Cloning Conch",
                    "Chef's Choice",
                    "Enchanted Lasso",
                    "Recruit a Trainee",
                    "Saloon's Finest",
                    "Lost Staff of Hamuul",
                    "Tomb Turning",
                )
                chosen = self._best_spell_target_positions(obs, plan, card, acts)
                # Macro B: Before casting `Butchering` with `Snazzy Phantom`, move best Undead right-most!
                if card.name == "Butchering":
                    prep = self._best_rightmost_undead_move(obs)
                    if prep is not None:
                        for pa in chosen[:1]:
                            out.append(((prep, pa), is_stoch))
                for pa in chosen:
                    out.append(((pa,), is_stoch))
            else:
                # Minion play: distinguish Magnetic vs Targeted Battlecry vs normal summon
                if card.magnetic and n_board > 0:
                    # Group magnetic plays (where board_pos < n_board and board[board_pos] is compatible)
                    mag_plays: List[Tuple[float, seaglass.TavernAction]] = []
                    non_mag_play: Optional[seaglass.TavernAction] = None
                    other_mechs_exist = any(
                        u.card_id != self._beatboxer_id
                        and ("mech" in str(u.tribe).lower() or str(u.tribe).lower() == "all")
                        for u in board
                    )
                    for pa in acts:
                        pos = pa.args[1]
                        if pos is not None and pos < n_board:
                            tgt = board[pos]
                            # NEVER magnetize directly onto Beatboxer when another Mech exists!
                            if has_beatboxer and tgt.card_id == self._beatboxer_id and other_mechs_exist:
                                continue
                            sc = (
                                (45.0 if tgt.card_id == self._duplicator_id else 0.0)
                                + (25.0 if tgt.divine_shield else 0.0)
                                + (18.0 if tgt.card_id in self.reg.cleave_ids else 0.0)
                                + 0.15 * (tgt.attack + tgt.health)
                                + 0.2 * plan.sell_protection(tgt)
                            )
                            mag_plays.append((sc, pa))
                        else:
                            non_mag_play = pa
                    mag_plays.sort(key=lambda x: x[0], reverse=True)
                    for _sc, pa in mag_plays[:2]:
                        out.append(((pa,), False))
                        # Macro C: Activate Drone Duplicator BEFORE magnetizing onto it!
                        if pa.args[1] is not None and pa.args[1] < n_board and board[pa.args[1]].card_id == self._duplicator_id:
                            dup_act = seaglass.TavernAction.activate(pa.args[1])
                            if game.is_legal(dup_act):
                                out.append(((dup_act, pa), False))
                    if non_mag_play is not None and (not mag_plays) and (n_board <= 2 or obs.turn <= 3):
                        out.append(((non_mag_play,), False))
                elif card.card_id in self._bc_target_ids:
                    # Targeted Battlecry (e.g., Elite Navigator, Captain Sanders, Maw Caster, Disguised Graverobber)
                    scored_bc = []
                    for pa in acts:
                        pos = pa.args[1]
                        if pos is not None and pos < n_board:
                            tgt = board[pos]
                            sc = plan.sell_protection(tgt) + 0.15 * (tgt.attack + tgt.health)
                            if card.name == "Elite Navigator" and tgt.card_id == self._conniver_id:
                                sc += 200.0  # Golden Greedy Conniver -> Tier 7 Discover!
                            elif card.name in ("Maw Caster", "Disguised Graverobber"):
                                # Target Plaguerunner / Eternal Summoner / Reborn Undead!
                                sc = (80.0 if tgt.name == "Plaguerunner" else 0.0) + (
                                    55.0 if tgt.reborn else 0.0
                                ) - 0.5 * plan.sell_protection(tgt)
                            scored_bc.append((sc, pa))
                        else:
                            scored_bc.append((0.0, pa))
                    scored_bc.sort(key=lambda x: x[0], reverse=True)
                    is_stoch = card.name in ("Maw Caster",)
                    for _sc, pa in scored_bc[:2]:
                        out.append(((pa,), is_stoch))
                else:
                    # Never play `Bream Counter` or `Flighty Scout` onto board when handbuff Murloc synergy is active!
                    if card.card_id in (self._bream_id, self._scout_id) and n_board >= 4:
                        has_hand_syn = any(
                            u.card_id
                            in (
                                self._choral_id,
                                self._costume_id,
                                self._diremuck_id,
                                self._futurefin_id,
                                self._tidehunter_id,
                            )
                            or "murloc" in str(u.tribe).lower()
                            for u in board
                        )
                        if has_hand_syn:
                            continue
                    # Standard minion placement: rightmost slot is canonical (positioning runs at EoT!)
                    is_stoch = card.card_id in self._choose_one_ids or card.name in (
                        "Primalfin Lookout",
                        "Rodeo Performer",
                        "Imposing Percussionist",
                        "Tavern Tempest",
                        "Auto Accelerator",
                        "Silent Deliverer",
                        "Draconic Warden",
                        "Shipwrecked Rascal",
                    )
                    out.append(((acts[-1],), is_stoch))

        # 2. Macro A: Full-Board Atomic `Sell(weakest) + Play(hand_minion)`
        if n_board >= 7:
            weak_slots = self._ranked_sell_slots(obs, plan, limit=2)
            for h_idx, card in enumerate(hand):
                if card.is_spell or card.unplayable or card.magnetic:
                    continue
                for w_pos in weak_slots:
                    sell_a = seaglass.TavernAction.sell(w_pos)
                    play_a = seaglass.TavernAction.play(h_idx, 6)
                    if game.is_legal(sell_a):
                        is_stoch = card.card_id in self._choose_one_ids or card.name in (
                            "Primalfin Lookout",
                            "Rodeo Performer",
                            "Imposing Percussionist",
                            "Tavern Tempest",
                            "Auto Accelerator",
                            "Silent Deliverer",
                        )
                        out.append(((sell_a, play_a), is_stoch))

        # 3. Activate abilities (with comp-specific prep macros!)
        activates = [a for a in legal if a.kind == "activate"]
        seen_act_src = set()
        for a in activates:
            src = a.args[0]
            tgt = a.args[1]
            if src is None or src >= n_board:
                continue
            unit = board[src]
            uname = unit.name

            # Macro B: Dead Bellringer -> prep Snazzy Phantom right-most Undead, target Plaguerunner/Reborn!
            if unit.card_id == self._bellringer_id and tgt is not None and tgt < n_board:
                t_unit = board[tgt]
                if plan.sell_protection(t_unit) > 35.0 and not t_unit.reborn and t_unit.name != "Plaguerunner":
                    continue
                prep = self._best_rightmost_undead_move(obs)
                if prep is not None:
                    out.append(((prep, a), False))
                out.append(((a,), False))
                continue

            # Macro E: Victorious Geomant -> if overflowing into slot 0, put Roogug/Amalgam left-most!
            if unit.card_id == self._geomant_id:
                if len(hand) >= 6 and not has_surveyor:
                    for idx, bu in enumerate(board):
                        if idx != 0 and bu.card_id in (self._roogug_id, self._amalgam_id):
                            out.append(((seaglass.TavernAction.reposition(idx, 0), a), False))
                            break
                out.append(((a,), False))
                continue

            # Macro I: Living Prison -> Activate(1) then Buy biggest shop minion!
            if unit.card_id == self._living_prison_id and obs.gold >= 1 + obs.minion_cost:
                best_shop_i = None
                best_sh_stat = 0
                for si, su in enumerate(shop):
                    if not su.is_spell and (su.attack + su.health) > best_sh_stat:
                        best_sh_stat = su.attack + su.health
                        best_shop_i = si
                if best_shop_i is not None:
                    out.append(((a, seaglass.TavernAction.buy(best_shop_i)), False))
                continue

            # Macro D: Lurking Lionfish -> move best Rally Beast to slot 0 first!
            if unit.card_id == self._lionfish_id and src not in seen_act_src:
                seen_act_src.add(src)
                best_rally_idx = None
                for idx, bu in enumerate(board):
                    if "beast" in str(bu.tribe).lower() and bu.card_id in self._rally_ids:
                        best_rally_idx = idx
                        break
                if best_rally_idx is not None and best_rally_idx != 0:
                    out.append(((seaglass.TavernAction.reposition(best_rally_idx, 0), a), False))
                out.append(((a,), False))
                continue

            # Hand-discard Activates (`Brain Rotter`, `Abyssal Envoy`, `Mindbending Recruiter`, `Mangled Bandit`, `N'raqi Frostcaller`)
            if tgt is not None and tgt < len(hand) and uname in (
                "Brain Rotter",
                "Abyssal Envoy",
                "Mindbending Recruiter",
                "Mangled Bandit",
                "N'raqi Frostcaller",
            ):
                if src in seen_act_src:
                    continue
                # Pick the BEST discard fodder: Sludge Corrosion first, then cheap spells, never core lynchpins!
                best_disc = None
                best_disc_sc = -1e18
                for da in activates:
                    if da.args[0] != src or da.args[1] is None or da.args[1] >= len(hand):
                        continue
                    hc = hand[da.args[1]]
                    if hc.name in self._sludge_names:
                        dsc = 120.0
                    elif hc.name == "Triple Reward" or hc.name == "Lockbox":
                        dsc = -200.0
                    elif hc.is_spell:
                        dsc = 25.0 - 3.0 * hc.tavern_tier
                    else:
                        dsc = 10.0 - plan.card_desire(hc.card_id, hc.name, hc.tavern_tier)
                    if dsc > best_disc_sc:
                        best_disc_sc = dsc
                        best_disc = da
                if best_disc is not None and best_disc_sc > -20.0:
                    seen_act_src.add(src)
                    is_stoch = uname in ("Abyssal Envoy", "Mindbending Recruiter")
                    out.append(((best_disc,), is_stoch))
                continue

            # General Activate (Deft Deserter, Soulkeeping Jailer, Kelp Keeper, Sky-hatch Runaway, Sewer Escapee, Fruit Vendor, Tyrael, etc.)
            if src not in seen_act_src:
                seen_act_src.add(src)
                is_stoch = uname in ("Clever Castaway", "Hired Mount")
                out.append(((a,), is_stoch))

        # 4. Buy actions from shop (ranked by comp desire + stats + pair/triple + scam!)
        #    PLUS atomic `Buy(spell) + Play(spell)`, Full-Board `Buy + Sell(weakest) + Play`,
        #    `Buy(magnetic) + Magnetize`, and `Arcane Behemoth` Elemental-feed macros!
        has_behemoth = any(u.card_id == self._behemoth_id for u in board)
        buys = [a for a in legal if a.kind == "buy" and a.args[0] is not None]
        if buys:
            scored_buys: List[Tuple[float, seaglass.TavernAction, seaglass.Unit]] = []
            for a in buys:
                si = a.args[0]
                if si >= len(shop):
                    continue
                scard = shop[si]
                desire = plan.card_desire(scard.card_id, scard.name, scard.tavern_tier, is_spell=scard.is_spell)
                if scard.is_spell:
                    cost = max(1, scard.spell_cost)
                    # Strong Tavern spells deserve top consideration when fused with casting!
                    spell_boost = 0.0
                    if scard.name in (
                        "Tavern Coin",
                        "Strike Oil",
                        "Careful Investment",
                        "Overconfidence",
                        "Hasty Excavation",
                        "Azerite Empowerment",
                        "Menagerie Tableware",
                        "Mighty Dragonbreath",
                        "Wave of Gold",
                        "Butchering",
                        "Corrupted Cupcakes",
                        "Methodical Madness",
                        "Staff of Enrichment",
                        "Eonar's Favor",
                        "Boon of Beetles",
                        "Temperature Shift",
                        "Upper Hand",
                        "Sharing is Caring",
                        "Hallowed Ritual",
                        "Eyes of the Earth Mother",
                        "Blood Gem Barrage",
                        "Easterly Winds",
                        "Fandral's Fortune",
                        "Cloning Conch",
                        "Boundless Potential",
                        "Wealthy Bounty",
                        "Friendly Bounty",
                        "Selfish Bounty",
                    ):
                        spell_boost = 11.0
                    sc = desire + spell_boost + 1.8 * scard.tavern_tier - 1.1 * cost
                    if scard.costs_health and not has_rewinder and (obs.health + obs.armor) <= 15:
                        sc -= 20.0
                    scored_buys.append((sc, a, scard))
                else:
                    stat_v = 0.45 * (scard.attack + scard.health) + 2.0 * scard.tavern_tier
                    triple_bonus = (
                        24.0
                        if plan.owned_non_golden_counts.get(scard.card_id, 0) >= 2
                        else (5.5 if plan.owned_non_golden_counts.get(scard.card_id, 0) == 1 else 0.0)
                    )
                    kw_bonus = (
                        (4.5 if scard.divine_shield else 0.0)
                        + (3.5 if scard.reborn else 0.0)
                        + (4.5 if scard.card_id in self.reg.cleave_ids else 0.0)
                    )
                    if has_behemoth and ("elemental" in str(scard.tribe).lower() or str(scard.tribe).lower() == "all"):
                        stat_v += 12.0 + 0.35 * (scard.attack + scard.health)
                    if plan.threats.scam_urgency >= 0.30 and (scard.venomous or scard.card_id in self.reg.scam_ids):
                        stat_v += 24.0 * plan.threats.scam_urgency
                    scored_buys.append((desire + stat_v + triple_bonus + kw_bonus, a, scard))

            scored_buys.sort(key=lambda x: x[0], reverse=True)
            weak_slots_full = self._ranked_sell_slots(obs, plan, limit=2) if n_board >= 7 else []
            weak_slot_full = weak_slots_full[0] if weak_slots_full else None
            best_mech_pos: Optional[int] = None
            if n_board > 0:
                best_m_sc = -1e18
                other_mechs = any(
                    u.card_id != self._beatboxer_id
                    and ("mech" in str(u.tribe).lower() or str(u.tribe).lower() == "all")
                    for u in board
                )
                for bi, bu in enumerate(board):
                    if "mech" in str(bu.tribe).lower() or str(bu.tribe).lower() == "all":
                        if has_beatboxer and bu.card_id == self._beatboxer_id and other_mechs:
                            continue
                        msc = (
                            (40.0 if bu.card_id == self._duplicator_id else 0.0)
                            + (20.0 if bu.divine_shield else 0.0)
                            + 0.15 * (bu.attack + bu.health)
                        )
                        if msc > best_m_sc:
                            best_m_sc = msc
                            best_mech_pos = bi

            next_hand_idx = len(hand)
            for _sc, a, scard in scored_buys[:6]:
                is_triple_buy = (
                    not scard.is_spell
                    and plan.owned_non_golden_counts.get(scard.card_id, 0) >= 2
                )
                is_stoch_play = (
                    scard.card_id in self._choose_one_ids
                    or scard.name
                    in (
                        "Primalfin Lookout",
                        "Rodeo Performer",
                        "Imposing Percussionist",
                        "Tavern Tempest",
                        "Auto Accelerator",
                        "Silent Deliverer",
                        "Draconic Warden",
                        "Shipwrecked Rascal",
                    )
                )
                if scard.is_spell:
                    # ATOMIC `Buy(spell) + Play(spell)` macro so Tavern spells aren't pruned at depth 1!
                    is_st_sp = scard.name in self._stochastic_spell_names
                    # Pick optimal board target (if targeted) or slot 0 (if untargeted)
                    tgt_pos = 0
                    if n_board > 0:
                        fake_plays = [seaglass.TavernAction.play(next_hand_idx, p) for p in range(n_board)]
                        best_ps = self._best_spell_target_positions(obs, plan, scard, fake_plays)
                        if best_ps:
                            tgt_pos = best_ps[0].args[1] or 0
                        # Tribe-specific target overrides for tribe-filtered spells
                        if scard.name == "Corrupted Cupcakes":
                            demons = [i for i, u in enumerate(board) if "demon" in str(u.tribe).lower() or str(u.tribe).lower() == "all"]
                            if demons:
                                tgt_pos = max(demons, key=lambda i: board[i].attack + board[i].health)
                        elif scard.name == "Butchering":
                            undeads = [i for i, u in enumerate(board) if "undead" in str(u.tribe).lower() or str(u.tribe).lower() == "all"]
                            if undeads:
                                tgt_pos = max(
                                    undeads,
                                    key=lambda i: (60.0 if board[i].reborn else 0.0) + (55.0 if board[i].name == "Plaguerunner" else 0.0) - plan.sell_protection(board[i]) * 0.4,
                                )
                        elif scard.name == "Sacred Gift":
                            non_ds = [i for i, u in enumerate(board) if not u.divine_shield]
                            if non_ds:
                                tgt_pos = max(non_ds, key=lambda i: board[i].attack + board[i].health + (40 if board[i].card_id in self.reg.cleave_ids else 0))
                    if not (harbinger_hold_payoff and scard.name in _HARBINGER_PAYOFF_SPELLS):
                        if scard.name == "Butchering":
                            prep = self._best_rightmost_undead_move(obs)
                            if prep is not None:
                                out.append(((prep, a, seaglass.TavernAction.play(next_hand_idx, tgt_pos)), is_st_sp))
                        out.append(((a, seaglass.TavernAction.play(next_hand_idx, tgt_pos)), is_st_sp))
                    out.append(((a,), False))
                elif is_triple_buy:
                    out.append(((a,), False))
                elif scard.magnetic and best_mech_pos is not None:
                    play_mag = seaglass.TavernAction.play(next_hand_idx, best_mech_pos)
                    if board[best_mech_pos].card_id == self._duplicator_id:
                        dup_a = seaglass.TavernAction.activate(best_mech_pos)
                        if game.is_legal(dup_a) and obs.gold >= obs.minion_cost + 1:
                            out.append(((dup_a, a, play_mag), False))
                    out.append(((a, play_mag), False))
                elif n_board < 7:
                    play_a = seaglass.TavernAction.play(next_hand_idx, n_board)
                    if scard.card_id in (self._bream_id, self._scout_id) and n_board >= 4:
                        out.append(((a,), False))
                    else:
                        out.append(((a, play_a), is_stoch_play))
                    is_elem_feed = has_behemoth and ("elemental" in str(scard.tribe).lower() or str(scard.tribe).lower() == "all")
                    if is_elem_feed or (
                        scard.card_id in self._sell_trigger_ids
                        and scard.name
                        in (
                            "Fire Baller",
                            "Snow Baller",
                            "Air Baller",
                            "Sellemental",
                            "Shoalfin Mystic",
                            "Wandering Willbreaker",
                            "Faceless Operative",
                        )
                    ):
                        sell_a = seaglass.TavernAction.sell(n_board)
                        is_st = scard.name in ("Wandering Willbreaker", "Faceless Operative")
                        out.append(((a, play_a, sell_a), is_st))
                else:
                    # Full board (n_board == 7): Atomic `Buy + Sell(weakest) + Play`!
                    # In Top-3 / Finals or when buying Scam, try top 2 weakest sell slots!
                    is_scam_buy = scard.venomous or scard.card_id in self.reg.scam_ids
                    slots_to_try = weak_slots_full if (is_scam_buy or plan.threats.is_finals) else weak_slots_full[:1]
                    for w_sl in slots_to_try:
                        if not scard.magnetic:
                            sell_a = seaglass.TavernAction.sell(w_sl)
                            play_a = seaglass.TavernAction.play(next_hand_idx, 6)
                            out.append(((a, sell_a, play_a), is_stoch_play))
                            is_elem_feed = has_behemoth and ("elemental" in str(scard.tribe).lower() or str(scard.tribe).lower() == "all")
                            if is_elem_feed or scard.name in (
                                "Fire Baller",
                                "Snow Baller",
                                "Air Baller",
                                "Sellemental",
                                "Shoalfin Mystic",
                            ):
                                out.append(((a, sell_a, play_a, seaglass.TavernAction.sell(6)), False))
                    if (
                        plan.owned_non_golden_counts.get(scard.card_id, 0) >= 1
                        or scard.name in ("Bream Counter", "Flighty Scout")
                    ):
                        out.append(((a,), False))

        # 4b. `play_on_shop` actions (Glambot on shop Mech, Shamanic Tidecaller on shop Murloc, Methodical Madness, etc.)
        pos_acts = [a for a in legal if a.kind == "play_on_shop"]
        if pos_acts:
            out.append(((pos_acts[0],), False))

        # 5. Upgrade Tavern (always consider when legal!)
        for a in legal:
            if a.kind == "upgrade_tavern":
                out.append(((a,), False))
                break

        # 6. High-value Sells:
        #    - Golden Greedy Conniver -> Tier 7 Discover!
        #    - Any non-Behemoth Elemental when `Arcane Behemoth` is on board!
        #    - True on-sell value minions (Ballers, Sellemental, Wandering Willbreaker, Faceless Operative, Snarky Shark, Tortollan Blue Shell, Shoalfin Mystic)
        for a in legal:
            if a.kind != "sell" or a.args[0] is None or a.args[0] >= n_board:
                continue
            pos = a.args[0]
            u = board[pos]
            if u.card_id == self._conniver_id and u.is_golden:
                out.append(((a,), True))
            elif has_behemoth and u.card_id != self._behemoth_id and ("elemental" in str(u.tribe).lower() or str(u.tribe).lower() == "all"):
                out.append(((a,), False))
            elif u.card_id in self._sell_trigger_ids and (
                plan.sell_protection(u) < 28.0
                or u.name in ("Fire Baller", "Snow Baller", "Air Baller", "Sellemental", "Tortollan Blue Shell", "Shoalfin Mystic")
            ):
                is_stoch = u.name in ("Wandering Willbreaker", "Faceless Operative", "Snarky Shark")
                out.append(((a,), is_stoch))

        # Allow selling the single weakest board minion when gold is 1 short of Upgrade or Buy
        if n_board >= 2:
            need_one_gold = (
                (obs.gold + 1 == obs.upgrade_cost and obs.tavern_tier < 6)
                or (obs.gold + 1 == obs.minion_cost and bool(shop))
            )
            if need_one_gold:
                w_pos = self._weakest_sell_slot(obs, plan)
                if w_pos is not None and plan.sell_protection(board[w_pos]) < 16.0:
                    sa = seaglass.TavernAction.sell(w_pos)
                    if game.is_legal(sa):
                        out.append(((sa,), False))

        # 7. Hero Power(s) — enumerate up to 3 best targets for targeted Hero Powers!
        hp_acts = [a for a in legal if a.kind in ("hero_power", "hero_power_2")]
        if hp_acts:
            if len(hp_acts) <= 3:
                for ha in hp_acts:
                    out.append(((ha,), True))
            else:
                scored_hp: List[Tuple[float, seaglass.TavernAction]] = []
                for ha in hp_acts:
                    tgt = ha.args[0]
                    sc = 0.0
                    if tgt is not None and tgt < n_board:
                        u = board[tgt]
                        sc = (u.attack + u.health) * 0.25 + plan.sell_protection(u)
                    elif tgt is not None and 7 <= tgt < 7 + len(shop):
                        su = shop[tgt - 7]
                        sc = plan.card_desire(su.card_id, su.name, su.tavern_tier) + 0.3 * (su.attack + su.health)
                    scored_hp.append((sc, ha))
                scored_hp.sort(key=lambda x: x[0], reverse=True)
                for _sc, ha in scored_hp[:3]:
                    out.append(((ha,), True))

        # 8. ToggleFreeze (only when shop has a genuinely desirable card we can't buy this turn, and hand isn't full)
        if not obs.is_frozen and shop and len(hand) < 9:
            has_freeze_worthy = any(
                (
                    not s.is_spell
                    and (
                        plan.card_desire(s.card_id, s.name, s.tavern_tier) >= 12.0
                        or plan.owned_non_golden_counts.get(s.card_id, 0) >= 2
                        or (s.attack + s.health) >= max(30, 6 * obs.turn)
                    )
                )
                for s in shop
            )
            if has_freeze_worthy:
                fr = seaglass.TavernAction.toggle_freeze()
                if game.is_legal(fr):
                    out.append(((fr,), False))

        return out

    # ------------------------------------------------------------------
    # CRN Rollout Evaluator for Stochastic Actions (Refresh, DarkDiscovery, Discover, HeroPower)
    # ------------------------------------------------------------------
    def _greedy_finish_epoch(
        self,
        sim: seaglass.TavernGame,
        plan: BuildPlan,
        rollout_seed: int,
        max_steps: int = 5,
    ) -> None:
        """
        Fast multi-roll greedy playout inside a simulated CRN branch after a stochastic action
        (Refresh, DarkDiscovery, Triple Reward, HeroPower). Chains up to 3 additional Refreshes
        past weak shops when `gold >= 4` so Tier-5/6 comp hunting isn't systematically undervalued!
        """
        rolls_used = 0
        actions_taken = 0
        for step_i in range(max_steps + 3):
            if actions_taken >= max_steps:
                break
            o = sim.observe()
            # Resolve any pending discover greedily
            if o.discover_pending is not None:
                disc = [a for a in sim.legal_actions() if a.kind == "choose_discover"]
                if not disc:
                    break
                best_da = disc[0]
                best_dv = -1e18
                for da in disc:
                    s2 = sim.copy()
                    s2.reseed(rollout_seed + step_i * 13)
                    s2.step(da)
                    o2 = s2.observe()
                    dv = self.evaluator.evaluate_fast(o2, plan, in_turn=True)
                    if len(o2.hand) > len(o.hand):
                        nc = o2.hand[-1]
                        dv += 0.9 * plan.card_desire(nc.card_id, nc.name, nc.tavern_tier, is_spell=nc.is_spell)
                        dv += sum(0.8 * self.evaluator.kb.dark_gift_weights.get(dg, 4.0) for dg in nc.dark_gifts)
                    if dv > best_dv:
                        best_dv = dv
                        best_da = da
                sim.step(best_da)
                continue

            cur_val = self.evaluator.evaluate_fast(o, plan, in_turn=True)
            best_macro: Optional[Tuple[seaglass.TavernAction, ...]] = None
            best_val = cur_val + 0.15

            # Check immediate hand plays (minions, magnetic, spells) & top 3 shop buys (including full-board swap!)
            legal = sim.legal_actions()
            n_board = len(o.board)
            cands: List[Tuple[seaglass.TavernAction, ...]] = []

            # 1. Play cards from hand
            for hi, hc in enumerate(o.hand):
                if hc.unplayable or hc.name == "Triple Reward":
                    continue
                if hc.is_spell:
                    p_acts = [a for a in legal if a.kind == "play" and a.args[0] == hi]
                    if p_acts:
                        cands.append((self._best_spell_target_positions(o, plan, hc, p_acts)[0],))
                elif hc.magnetic and n_board > 0:
                    p_acts = [a for a in legal if a.kind == "play" and a.args[0] == hi and a.args[1] is not None and a.args[1] < n_board]
                    if p_acts:
                        cands.append((p_acts[0],))
                elif n_board < 7:
                    p_acts = [a for a in legal if a.kind == "play" and a.args[0] == hi]
                    if p_acts:
                        cands.append((p_acts[-1],))
                else:
                    w_pos = self._weakest_sell_slot(o, plan)
                    if w_pos is not None:
                        cands.append((seaglass.TavernAction.sell(w_pos), seaglass.TavernAction.play(hi, 6)))

            # 2. Free / cheap Activates (Geomant, Deft Deserter, Jailer, Fruit Vendor, Kelp Keeper, etc.)
            seen_g_act = set()
            for a in legal:
                if a.kind == "activate" and a.args[0] is not None and a.args[0] not in seen_g_act:
                    seen_g_act.add(a.args[0])
                    cands.append((a,))
                    if len(seen_g_act) >= 2:
                        break

            # 3. Top 3 shop buys (+ immediate play/swap)
            if o.gold >= 1 and len(o.hand) < 10:
                buys = [a for a in legal if a.kind == "buy" and a.args[0] is not None and a.args[0] < len(o.shop)]
                buys.sort(
                    key=lambda a: (
                        plan.card_desire(
                            o.shop[a.args[0]].card_id,
                            o.shop[a.args[0]].name,
                            o.shop[a.args[0]].tavern_tier,
                            is_spell=o.shop[a.args[0]].is_spell,
                        )
                        + 0.45 * (o.shop[a.args[0]].attack + o.shop[a.args[0]].health)
                        + (20.0 if plan.owned_non_golden_counts.get(o.shop[a.args[0]].card_id, 0) >= 2 else 0.0)
                    ),
                    reverse=True,
                )
                nhi = len(o.hand)
                w_pos = self._weakest_sell_slot(o, plan) if n_board >= 7 else None
                for ba in buys[:4]:
                    scard = o.shop[ba.args[0]]
                    if scard.is_spell:
                        tgt_p = 0
                        if n_board > 0:
                            fp = [seaglass.TavernAction.play(nhi, p) for p in range(n_board)]
                            bps = self._best_spell_target_positions(o, plan, scard, fp)
                            if bps:
                                tgt_p = bps[0].args[1] or 0
                        cands.append((ba, seaglass.TavernAction.play(nhi, tgt_p)))
                        cands.append((ba,))
                    elif n_board < 7:
                        cands.append((ba, seaglass.TavernAction.play(nhi, n_board)))
                    elif w_pos is not None and not scard.magnetic:
                        cands.append((ba, seaglass.TavernAction.sell(w_pos), seaglass.TavernAction.play(nhi, 6)))
                    else:
                        cands.append((ba,))

            for macro in cands:
                s2 = sim.copy()
                s2.reseed(rollout_seed + step_i * 29)
                ok = True
                for ma in macro:
                    if not s2.is_legal(ma):
                        ok = False
                        break
                    s2.step(ma)
                    if s2.observe().discover_pending is not None:
                        break
                if not ok:
                    continue
                v2 = self.evaluator.evaluate_fast(s2.observe(), plan, in_turn=True)
                if v2 > best_val:
                    best_val = v2
                    best_macro = macro

            if best_macro is None:
                # Multi-roll chaining inside CRN rollout: if current shop had nothing worth buying,
                # roll up to 3 more times while we still have gold to buy after rolling!
                can_chain_roll = (
                    rolls_used < 3
                    and o.turn >= 4
                    and (o.free_refreshes > 0 or o.gold >= o.refresh_cost + o.minion_cost)
                )
                if can_chain_roll:
                    ref_a = seaglass.TavernAction.refresh()
                    if sim.is_legal(ref_a):
                        sim.step(ref_a)
                        rolls_used += 1
                        continue
                break
            for ma in best_macro:
                if sim.is_legal(ma):
                    sim.step(ma)
                    if sim.observe().discover_pending is not None:
                        break
            actions_taken += 1

        # Ensure no discover/trinket prompt is left pending before `end_turn()`!
        for _ in range(4):
            o = sim.observe()
            if o.discover_pending is None and not o.trinket_options:
                break
            prompts = [a for a in sim.legal_actions() if a.kind in ("choose_discover", "choose_trinket")]
            if not prompts:
                break
            sim.step(prompts[0])

    def _eval_stochastic_crn(
        self,
        base_game: seaglass.TavernGame,
        actions: Tuple[seaglass.TavernAction, ...],
        plan: BuildPlan,
        opp_board: Sequence[seaglass.Unit],
        opp_tier: int,
        base_seed: int,
        k_samples: int = 2,
        ctx: Optional[seaglass.LobbyContext] = None,
        particles: Optional[SovereignOpponentParticles] = None,
    ) -> float:
        """Evaluate a stochastic action sequence averaged over `k_samples` CRN rollouts."""
        total = 0.0
        valid = 0
        for ki in range(k_samples):
            s_seed = (base_seed + ki * 9973 + 17) & 0x7FFFFFFF
            sim = base_game.copy()
            sim.reseed(s_seed)
            ok = True
            for a in actions:
                if not sim.is_legal(a):
                    ok = False
                    break
                sim.step(a)
            if not ok:
                continue
            self._greedy_finish_epoch(sim, plan, s_seed + 101, max_steps=5)
            total += self.evaluator.evaluate_turn_end(
                sim,
                plan,
                opp_board,
                opp_tier,
                seed=base_seed,
                rollouts=max(6, self.leaf_rollouts // 2),
                ctx=ctx,
                particles=particles,
            )
            valid += 1
        return (total / valid) if valid > 0 else -1e18

    # ------------------------------------------------------------------
    # Shop-Epoch Beam Search + Exact 1-Turn Physics Evaluation
    # ------------------------------------------------------------------
    def _plan_shop_epoch(
        self,
        base_game: seaglass.TavernGame,
        obs: seaglass.TavernObservation,
        plan: BuildPlan,
        ctx: Optional[seaglass.LobbyContext] = None,
        particles: Optional[SovereignOpponentParticles] = None,
    ) -> List[seaglass.TavernAction]:
        opp_board, opp_tier = self.positioner.build_opponent_board(obs, plan)
        sim_seed = (self.seed ^ (obs.turn * 1000003) ^ (self._rolls_this_turn * 97)) & 0x7FFFFFFF

        # Immediate priority: if we hold a `Triple Reward` at Tier 4+ (or hand >= 8), cast it FIRST so we can plan around what we discover!
        if obs.tavern_tier >= 4 or len(obs.hand) >= 8:
            for hi, hc in enumerate(obs.hand):
                if hc.is_spell and hc.name == "Triple Reward":
                    tr_act = seaglass.TavernAction.play(hi, 0)
                    if base_game.is_legal(tr_act):
                        return [tr_act]

        root_val = self.evaluator.evaluate_fast(obs, plan, in_turn=True)
        root = _BeamNode(game=base_game, obs=obs, actions=(), fast_val=root_val)

        beam: List[_BeamNode] = [root]
        terminal_pool: Dict[Tuple, _BeamNode] = {_state_sig(obs): root}
        stochastic_pool: List[_BeamNode] = []

        for _depth in range(self.max_depth):
            next_beam_map: Dict[Tuple, _BeamNode] = {}
            for node in beam:
                if node.stochastic_done:
                    continue
                cands = self._candidate_macros(node.game, node.obs, plan)
                for macro_acts, is_stoch in cands:
                    sim = node.game.copy()
                    sim.reseed(sim_seed + _depth * 31)
                    ok = True
                    for ma in macro_acts:
                        if not sim.is_legal(ma):
                            ok = False
                            break
                        sim.step(ma)
                        if sim.observe().discover_pending is not None:
                            is_stoch = True
                            break
                    if not ok:
                        continue

                    n_obs = sim.observe()
                    n_val = self.evaluator.evaluate_fast(n_obs, plan, in_turn=True)
                    if n_obs.discover_pending is not None:
                        n_val += 18.0 + 2.5 * n_obs.tavern_tier
                    new_node = _BeamNode(
                        game=sim,
                        obs=n_obs,
                        actions=node.actions + macro_acts,
                        fast_val=n_val,
                        stochastic_done=is_stoch,
                    )
                    if is_stoch:
                        stochastic_pool.append(new_node)
                    else:
                        sig = _state_sig(n_obs)
                        prev = next_beam_map.get(sig)
                        if prev is None or n_val > prev.fast_val:
                            next_beam_map[sig] = new_node
                        prev_t = terminal_pool.get(sig)
                        if prev_t is None or n_val > prev_t.fast_val:
                            terminal_pool[sig] = new_node

            if not next_beam_map:
                break
            beam = sorted(next_beam_map.values(), key=lambda x: x.fast_val, reverse=True)[
                : self.beam_width
            ]

        # Score the top K deterministic terminal states with EXACT 1-turn combat + start_turn() Rust physics!
        top_terminals = sorted(
            terminal_pool.values(),
            key=lambda x: self.evaluator.evaluate_fast(x.obs, plan, in_turn=False),
            reverse=True,
        )[: self.leaf_eval_k]
        if root not in top_terminals:
            top_terminals.append(root)

        best_end_node = root
        best_end_val = -1e18
        for node in top_terminals:
            val = self.evaluator.evaluate_turn_end(
                node.game,
                plan,
                opp_board,
                opp_tier,
                seed=sim_seed,
                rollouts=self.leaf_rollouts,
                ctx=ctx,
                particles=particles,
            )
            if val > best_end_val:
                best_end_val = val
                best_end_node = node

        # Compete EVERY major stochastic branch on the EXACT SAME turn-end physics scale via CRN rollouts!
        best_choice_acts: Optional[List[seaglass.TavernAction]] = None
        best_choice_val = best_end_val

        # (A) Top stochastic candidates discovered in beam (Discover, HeroPower, Golden Conniver sell, Choose One)
        if stochastic_pool:
            stochastic_pool.sort(key=lambda x: x.fast_val, reverse=True)
            seen_first_stoch = set()
            checked = 0
            for snode in stochastic_pool:
                key = tuple(_action_key(a) for a in snode.actions)
                if key in seen_first_stoch:
                    continue
                seen_first_stoch.add(key)
                sval = self._eval_stochastic_crn(
                    base_game,
                    snode.actions,
                    plan,
                    opp_board,
                    opp_tier,
                    sim_seed + 53,
                    k_samples=3,
                    ctx=ctx,
                    particles=particles,
                )
                if sval > best_choice_val + 0.30:
                    best_choice_val = sval
                    best_choice_acts = list(snode.actions)
                checked += 1
                if checked >= 4:
                    break

        # (B) Dark Discovery (3g, 3 uses/game from Turn 3) — compete directly against buying current shop!
        if obs.dark_discovery_usable and obs.gold >= 3 and (obs.turn >= 4 or obs.tavern_tier >= 2):
            dd_act = seaglass.TavernAction.dark_discovery()
            if base_game.is_legal(dd_act):
                dd_val = self._eval_stochastic_crn(
                    base_game,
                    (dd_act,),
                    plan,
                    opp_board,
                    opp_tier,
                    sim_seed + 109,
                    k_samples=3,
                    ctx=ctx,
                    particles=particles,
                )
                dd_val += 2.0 if obs.tavern_tier >= 3 else 0.5
                if dd_val > best_choice_val + 0.25:
                    best_choice_val = dd_val
                    best_choice_acts = [dd_act]

        # (C) Stratified Refresh Prefixes: root + best upgrade prefix + top 2 play/activate prefixes!
        if self._rolls_this_turn < 10:
            ref_act = seaglass.TavernAction.refresh()
            ref_prefixes: List[_BeamNode] = [root]
            added_up = False
            play_prefixes = 0
            for cand_n in sorted(terminal_pool.values(), key=lambda x: x.fast_val, reverse=True):
                if cand_n is root or not cand_n.actions:
                    continue
                co = cand_n.obs
                if not (co.free_refreshes > 0 or co.gold >= co.refresh_cost + co.minion_cost):
                    continue
                did_upgrade = co.tavern_tier > obs.tavern_tier
                if did_upgrade and not added_up:
                    ref_prefixes.append(cand_n)
                    added_up = True
                elif not did_upgrade and play_prefixes < 2:
                    ref_prefixes.append(cand_n)
                    play_prefixes += 1
                if added_up and play_prefixes >= 2:
                    break

            for pnode in ref_prefixes:
                po = pnode.obs
                has_free = po.free_refreshes > 0
                has_malch_rew = (
                    any(u.card_id == self._malchezaar_id for u in po.board)
                    and any(u.card_id in (self._rewinder_id, self._corruptor_id) for u in po.board)
                    and (po.health + po.armor) >= 10
                )
                eff_cost = 0 if (has_free or has_malch_rew) else po.refresh_cost
                if eff_cost > 0 and (po.turn <= 3 or po.gold < eff_cost + po.minion_cost):
                    continue
                if not pnode.game.is_legal(ref_act):
                    continue
                r_acts = pnode.actions + (ref_act,)
                r_val = self._eval_stochastic_crn(
                    base_game,
                    r_acts,
                    plan,
                    opp_board,
                    opp_tier,
                    sim_seed + 211,
                    k_samples=4 if eff_cost > 0 else 2,
                    ctx=ctx,
                    particles=particles,
                )
                if eff_cost == 0:
                    r_val += 1.5
                if r_val > best_choice_val + 0.30:
                    best_choice_val = r_val
                    best_choice_acts = list(r_acts)

        if best_choice_acts is not None:
            return best_choice_acts

        # Deterministic best_end_node won!
        if best_end_node.actions:
            post_obs = best_end_node.obs
            if (
                post_obs.free_refreshes > 0
                or (post_obs.dark_discovery_usable and post_obs.gold >= 3 and post_obs.turn >= 4)
                or (post_obs.gold >= post_obs.refresh_cost + post_obs.minion_cost and post_obs.turn >= 4 and self._rolls_this_turn < 10)
            ):
                return list(best_end_node.actions)
            return self._append_final_positioning(best_end_node, plan, sim_seed, particles=particles)

        # Peek-and-freeze with 1 leftover gold on Turn >= 5 if not frozen
        if (
            obs.turn >= 5
            and obs.gold == obs.refresh_cost
            and not obs.is_frozen
            and self._rolls_this_turn < 10
        ):
            ref_act = seaglass.TavernAction.refresh()
            if base_game.is_legal(ref_act):
                return [ref_act]

        return self._append_final_positioning(best_end_node, plan, sim_seed, particles=particles)

    def _append_final_positioning(
        self,
        end_node: _BeamNode,
        plan: BuildPlan,
        sim_seed: int,
        particles: Optional[SovereignOpponentParticles] = None,
    ) -> List[seaglass.TavernAction]:
        seq = list(end_node.actions)
        moves = self.positioner.optimize_end_of_turn(
            end_node.game,
            plan,
            seed=sim_seed,
            max_candidates=self.pos_candidates,
            rollouts_per_cand=self.pos_rollouts,
            particles=particles,
        )
        for src, dst in moves:
            seq.append(seaglass.TavernAction.reposition(src, dst))
        seq.append(seaglass.TavernAction.end_turn())
        return seq
