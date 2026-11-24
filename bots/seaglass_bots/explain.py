"""Explain a SovereignBot seat's decisions in a recorded solo game.

    python -m seaglass_bots.explain REPLAY --player p0 --turn 5 [--what-if] [--activations OUT.npz]

The replay does not store the bot's state, so the seat is re-run: the lobby is rebuilt from the replay header,
the recorded actions are applied in order, and the traced seat's `SovereignBot` (same constructor seed) is asked
for its action at each of its decisions. Its RNG streams then match the live run, so the plans, scores and
gates it prints are the ones that produced the recorded actions. If the bot's action differs from the recorded
one the run stops (nothing it printed after that point could be trusted) unless `--keep-going`.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from typing import Any, Dict, List, Optional, Sequence, Tuple

import numpy as np
import seaglass

from .sovereign_search import SovereignBot, _action_key
from .sovereign_trace import DecisionTrace, PlanTrace, key_to_action

ActionKey = Tuple[str, Optional[int], Optional[int]]

SOLO_ROLES = ["Search", "Search", "SemiRandom", "SemiRandom", "Tempo", "Tempo", "GreedyLevel", "GreedyLevel"]
WHAT_IF_SEEDS = [((0x5EED + 7919 * i) * 2654435761 % (1 << 62)) | 1 for i in range(6)]


class ExplainError(Exception):
    pass


# ---------------------------------------------------------------------------------------------
# Inputs: player, bot seed, lobby
# ---------------------------------------------------------------------------------------------


def resolve_player(spec: str, names: Sequence[str]) -> int:
    s = str(spec).strip()
    if re.fullmatch(r"\d+", s) and int(s) < len(names):
        return int(s)
    m = re.fullmatch(r"[pP](\d+)", s)
    if m and int(m.group(1)) < len(names):
        return int(m.group(1))
    if s in names:
        return names.index(s)
    hits = [i for i, n in enumerate(names) if s.lower() in n.lower()]
    if len(hits) == 1:
        return hits[0]
    raise ExplainError(f"cannot resolve --player {spec!r} among {list(names)}")


def infer_bot_seed(header: Dict[str, Any], player: int, bot_seed: Optional[int]) -> Tuple[int, str]:
    """(seed, where it came from): --bot-seed, replay meta "seats", or the solo arena convention."""
    if bot_seed is not None:
        return int(bot_seed), "--bot-seed"
    meta = header.get("meta") or {}
    if "seats" in meta:
        try:
            for ent in json.loads(meta["seats"]):
                if int(ent["seat"]) == player and ent.get("seed") is not None:
                    return int(ent["seed"]), "replay meta seats"
        except (ValueError, KeyError, TypeError):
            pass
    names = [p["name"] for p in header["players"]]
    n = len(names)
    shared = int(header["shared_seed"])
    if n == 8:
        sear = {i for i, nm in enumerate(names) if nm.startswith("Search(")}
        for rot in range(8):
            if sear == {rot, (rot + 1) % 8} and all(
                names[(i + rot) % 8].startswith(SOLO_ROLES[i]) for i in range(8)
            ):
                logical = (player - rot) % 8
                return shared + 13 * logical, f"arena convention (rotation {rot}, logical seat {logical})"
    raise ExplainError("cannot infer the bot's seed from this replay; pass --bot-seed")


def build_lobby(header: Dict[str, Any]) -> seaglass.LobbyGame:
    if header.get("duos") or header.get("pairs") or header.get("teams"):
        raise ExplainError("Duos replays are not supported yet (solo only)")
    players = header["players"]
    shared = int(header["shared_seed"])
    for i, p in enumerate(players):
        if int(p["seed"]) != shared + i:
            raise ExplainError(
                f"player {i} seed {p['seed']} != shared_seed + {i}; LobbyGame cannot set per-player seeds"
            )
    return seaglass.LobbyGame(
        players=len(players),
        seed=shared,
        names=[p["name"] for p in players],
        turn_timer=False,
    )


# ---------------------------------------------------------------------------------------------
# Readable names
# ---------------------------------------------------------------------------------------------


_TARGETED: Optional[set] = None


def _targeted_spell(card_id: int) -> bool:
    """Whether a spell is cast on a board minion (`seaglass.card_tags`, tag `targeted_spell`)."""
    global _TARGETED
    if _TARGETED is None:
        try:
            names, table = seaglass.card_tags()
            i = list(names).index("targeted_spell")
            _TARGETED = {cid for cid, row in table.items() if row[i] > 0.5}
        except Exception:
            _TARGETED = set()
    return card_id in _TARGETED


def _nm(seq: Sequence[Any], i: Optional[int]) -> str:
    if i is None or i < 0 or i >= len(seq):
        return f"#{i}"
    return seq[i].name


def describe_action(key: ActionKey, obs: seaglass.TavernObservation) -> str:
    kind, a0, a1 = key
    if kind == "buy":
        return f"Buy {_nm(obs.shop, a0)}"
    if kind == "play":
        card = obs.hand[a0] if a0 is not None and 0 <= a0 < len(obs.hand) else None
        if card is not None and card.is_spell:
            if _targeted_spell(card.card_id) and a1 is not None and 0 <= a1 < len(obs.board):
                return f"Cast {card.name} on {obs.board[a1].name}"
            return f"Cast {card.name}"
        return f"Play {_nm(obs.hand, a0)} at {a1}"
    if kind == "sell":
        return f"Sell {_nm(obs.board, a0)}"
    if kind == "reposition":
        return f"Move {_nm(obs.board, a0)} to {a1}"
    if kind == "activate":
        t = "" if a1 is None else f" on {_nm(obs.board, a1)}"
        return f"Activate {_nm(obs.board, a0)}{t}"
    if kind == "play_on_shop":
        return f"Play {_nm(obs.hand, a0)} on shop {_nm(obs.shop, a1)}"
    if kind == "upgrade_tavern":
        return f"Upgrade to tier {obs.tavern_tier + 1}"
    if kind == "toggle_freeze":
        return "Unfreeze" if obs.is_frozen else "Freeze"
    if kind == "end_turn":
        return "End turn"
    if kind == "refresh":
        return "Roll"
    if kind == "dark_discovery":
        return "Dark Discovery"
    if kind == "hero_power":
        return "Hero power" + ("" if a0 is None else f" (target {a0})")
    if kind == "hero_power_2":
        return "Hero power 2" + ("" if a0 is None else f" (target {a0})")
    if kind == "choose_discover" and obs.discover_pending:
        return f"Discover {_nm(obs.discover_pending, a0)}"
    return index_form(key)


def index_form(key: ActionKey) -> str:
    """A step named by position only (`Buy shop[2]`), for steps after a random outcome."""
    kind, a0, a1 = key
    if kind == "buy":
        return f"Buy shop[{a0}]"
    if kind == "play":
        return f"Play hand[{a0}] at {a1}"
    if kind == "sell":
        return f"Sell board[{a0}]"
    if kind == "reposition":
        return f"Move board[{a0}] to {a1}"
    if kind == "play_on_shop":
        return f"Cast hand[{a0}] on shop[{a1}]"
    if kind == "activate":
        return f"Activate board[{a0}]" + ("" if a1 is None else f" on {a1}")
    if kind == "choose_discover":
        return f"Choose option {a0}"
    fixed = {"refresh": "Roll", "upgrade_tavern": "Upgrade tavern", "toggle_freeze": "Toggle freeze",
             "end_turn": "End turn", "dark_discovery": "Dark Discovery"}
    if kind in fixed:
        return fixed[kind]
    args = ",".join(str(x) for x in (a0, a1) if x is not None)
    return f"{kind}({args})" if args else kind


_ALT_SEEDS = (0x5DEECE66D, 0x2545F4914F6CDD1D, 0x9E3779B97F4A7C15, 0xD1B54A32D192ED03)


def _visible(obs: seaglass.TavernObservation) -> Tuple:
    cards = lambda units: tuple((u.card_id, u.attack, u.health) for u in units)  # noqa: E731
    return (cards(obs.board), cards(obs.hand), cards(obs.shop), obs.discover_pending is not None)


def describe_line(game: Optional[seaglass.TavernGame], keys: Sequence[ActionKey]) -> str:
    """Steps by name, found by stepping a copy of `game`; index form once the copy can no longer be trusted
    (after a roll, discover or Dark Discovery, a step whose outcome depends on the random draw, or an
    illegal step), because the bot planned the rest on its own sampled outcome."""
    out: List[str] = []
    g = game.copy() if game is not None else None
    trusted = g is not None
    obs = g.observe() if trusted else None
    for key in keys:
        key = tuple(key)  # type: ignore[assignment]
        if not trusted or obs is None:
            out.append(index_form(key))  # type: ignore[arg-type]
            continue
        out.append(describe_action(key, obs))  # type: ignore[arg-type]
        if key[0] in ("refresh", "dark_discovery"):
            trusted = False
            continue
        try:
            act = key_to_action(key)
            if not g.is_legal(act):
                trusted = False
                continue
            # Step copies with other draws too: if any differs, the outcome was random. (A random pick
            # from a few options can coincide, hence several seeds.)
            alts = []
            for seed in _ALT_SEEDS:
                alt = g.copy()
                alt.reseed(seed)
                alt.step(act)
                alts.append(_visible(alt.observe()))
            g.step(act)
            obs = g.observe()
            if (
                obs.discover_pending is not None
                or obs.trinket_options
                or any(v != _visible(obs) for v in alts)
            ):
                trusted = False
        except Exception:
            trusted = False
    return " > ".join(out)


def _units(units: Sequence[Any]) -> str:
    return ", ".join(f"{u.name} {u.attack}/{u.health}" for u in units) or "-"


def _fmt(x: Optional[float], w: int = 7, d: int = 2) -> str:
    return " " * (w - 1) + "-" if x is None else f"{x:{w}.{d}f}"


# ---------------------------------------------------------------------------------------------
# Printing one decision
# ---------------------------------------------------------------------------------------------


def format_decision(
    frame: int,
    player: int,
    obs: seaglass.TavernObservation,
    trace: DecisionTrace,
    matches: bool,
) -> str:
    L: List[str] = []
    L.append("=" * 100)
    L.append(
        f"Frame {frame} | p{player} turn {obs.turn} | gold {obs.gold}/{obs.max_gold} | tier {obs.tavern_tier}"
        f" | frozen {'yes' if obs.is_frozen else 'no'} | hp {obs.health}+{obs.armor}"
    )
    L.append(f"  board: {_units(obs.board)}")
    L.append(f"  hand:  {_units(obs.hand)}")
    L.append(f"  shop:  {_units(obs.shop)}")
    chosen_txt = describe_action(trace.chosen, obs) if trace.chosen else "-"
    L.append(f"Decision: {trace.kind} -> {chosen_txt}   {'' if matches else '(DIFFERS FROM RECORDING)'}")
    if trace.queued is not None:
        pv = trace.queued.get("plan_value")
        L.append(
            f"  following the queued '{trace.queued.get('label')}' plan"
            + (f" (value {pv:.2f})" if pv is not None else "")
            + f", {trace.queued.get('steps_left')} steps left after this one"
        )
    if trace.plans:
        L.append(
            f"  best value {trace.value:.2f} (epoch best {trace.epoch_ev:.2f}); a roll / Dark Discovery / "
            f"hero power must beat the best plan by {trace.margin:.2f}"
        )
        if trace.counts:
            L.append("  search: " + ", ".join(f"{k} {v}" for k, v in trace.counts.items()))
        L.append("")
        L.append(
            f"  {'score':>8} {'label':<15}{'':<2}{'combat':>8}{'nxt prior':>10}{'nxt net':>9}{'discipl':>9}  line"
        )
        for p in sorted(trace.plans, key=lambda p: p.score, reverse=True):
            pr = p.parts
            line = describe_line(trace.game, p.actions)
            L.append(
                f"  {p.score:8.2f} {p.label:<15}{'*' if p.chosen else ' ':<2}"
                f"{_fmt(pr.get('combat'), 8)}{_fmt(pr.get('next_prior'), 10)}{_fmt(pr.get('next_net'), 9)}"
                f"{_fmt(pr.get('discipline'), 9)}  {line}"
            )
    if trace.gates:
        L.append("")
        L.append("  Gates:")
        for g in trace.gates:
            L.append(f"    [{'pass   ' if g.passed else 'blocked'}] {g.name}: {g.detail}")
    if trace.finalize_rule is not None:
        L.append(f"  Finalize rule: {trace.finalize_rule}")
    return "\n".join(L)


# ---------------------------------------------------------------------------------------------
# What-ifs
# ---------------------------------------------------------------------------------------------


def blocked_refresh(trace: DecisionTrace) -> Optional[str]:
    g = trace.gate("root_refresh")
    if g is not None and g.values.get("legal") and not g.passed:
        return "root_refresh"
    z = trace.gate("zero_waste_refresh")
    if z is not None and z.values.get("legal") and not z.passed:
        return "zero_waste_refresh"
    return None


def run_what_ifs(bot: SovereignBot, trace: DecisionTrace) -> Dict[str, Any]:
    """Score alternatives on the decision's own game / particles / ctx with 6 fixed CRN seeds.

    Works on copies and restores `bot.rng` / `bot.belief.rng` afterwards.
    """
    game, particles, ctx = trace.game, trace.particles, trace.ctx
    if game is None or particles is None:
        return {}
    ev = bot.evaluator
    rng_state = bot.rng.getstate()
    belief_rng_state = bot.belief.rng.getstate()
    out: Dict[str, Any] = {}
    refresh = seaglass.TavernAction.refresh()
    freeze = seaglass.TavernAction.toggle_freeze()
    base = trace.value
    if base is None and trace.queued is not None:
        base = trace.queued.get("plan_value")
    try:
        # (a) end turn now
        parts: Dict[str, float] = {}
        sc = ev.evaluate_turn_end(game.copy(), particles, ctx=ctx, combats_per_particle=14, parts=parts)
        out["end_turn"] = {"score": float(sc), "parts": parts}
        # The bot scores its plans with 14 combats per particle but rolls with 8 (`_evaluate_stochastic_action`),
        # so also score ending now with 8: the like-for-like baseline for the roll lines.
        parts = {}
        sc = ev.evaluate_turn_end(game.copy(), particles, ctx=ctx, combats_per_particle=8, parts=parts)
        out["end_turn_8"] = {"score": float(sc), "parts": parts}

        # (b) freeze, then end turn
        if game.is_legal(freeze):
            g2 = game.copy()
            g2.step(freeze)
            parts = {}
            sc = ev.evaluate_turn_end(g2, particles, ctx=ctx, combats_per_particle=14, parts=parts)
            out["freeze_end_turn"] = {"score": float(sc), "parts": parts}

        if game.is_legal(refresh):
            # (c) roll, scored the way the bot scores a root Refresh
            parts = {}
            sc = bot._evaluate_stochastic_action(
                game, refresh, particles, ctx, crn_seeds=WHAT_IF_SEEDS, n_samples=len(WHAT_IF_SEEDS), parts=parts
            )
            out["roll"] = {"score": float(sc), "parts": parts, "samples": len(WHAT_IF_SEEDS)}

            # (d) roll, then freeze if that scores better
            total = 0.0
            psum: Dict[str, float] = {}
            n_frozen = 0
            for s_val in WHAT_IF_SEEDS:
                sim = game.copy()
                sim.reseed(s_val)
                sim.step(refresh)
                legal_after = sim.legal_actions()
                if legal_after and legal_after[0].kind == "choose_discover":
                    pick = bot._evaluate_discover_options(
                        sim, legal_after, particles, ctx, allow_shop_continuation=False
                    )
                    sim.step(pick)
                    bot._play_newly_gained_card_in_sim(sim, particles=particles, ctx=ctx)
                sim = bot._rollout_shop_consumer(
                    sim, ctx, particles=particles, max_steps=4, crn_seed=(s_val ^ 0x9E3779B97F4A7C15) | 1
                )
                p0: Dict[str, float] = {}
                v0 = ev.evaluate_turn_end(sim.copy(), particles, ctx=ctx, combats_per_particle=8, parts=p0)
                best_v, best_p = v0, p0
                if sim.is_legal(freeze):
                    sf = sim.copy()
                    sf.step(freeze)
                    p1: Dict[str, float] = {}
                    v1 = ev.evaluate_turn_end(sf, particles, ctx=ctx, combats_per_particle=8, parts=p1)
                    if v1 > v0:
                        best_v, best_p = v1, p1
                        n_frozen += 1
                total += best_v
                for k, v in best_p.items():
                    psum[k] = psum.get(k, 0.0) + v
            n = len(WHAT_IF_SEEDS)
            out["roll_then_freeze"] = {
                "score": total / n,
                "parts": {k: v / n for k, v in psum.items()},
                "samples": n,
                "frozen_in": n_frozen,
            }
    finally:
        bot.rng.setstate(rng_state)
        bot.belief.rng.setstate(belief_rng_state)
    if base is not None:
        for v in out.values():
            v["gap_to_chosen"] = float(v["score"] - base)
    if "end_turn_8" in out:
        base8 = out["end_turn_8"]["score"]
        for k in ("end_turn_8", "roll", "roll_then_freeze"):
            if k in out:
                out[k]["gap_to_end_turn_8"] = float(out[k]["score"] - base8)
    return out


LABELS = {
    "end_turn": "end turn now",
    "end_turn_8": "end turn now, 8 combats (how rolls are scored)",
    "freeze_end_turn": "freeze, end turn",
    "roll": "roll (bot's root-refresh scoring)",
    "roll_then_freeze": "roll, then freeze if better",
}


def format_what_if(trace: DecisionTrace, wi: Dict[str, Any], why: str) -> str:
    base = trace.value
    if base is None and trace.queued is not None:
        base = trace.queued.get("plan_value")
    base_txt = "n/a" if base is None else f"{base:.2f}"
    L = [
        f"  What-if ({why}; {len(WHAT_IF_SEEDS)} fixed CRN seeds; 'gap' is to the chosen line's value {base_txt};",
        "  'vs end(8)' compares like for like: the bot scores rolls with 8 combats per opponent board, plans with 14):",
    ]
    L.append(
        f"    {'score':>8} {'gap':>7} {'vs end(8)':>9} {'combat':>8}{'nxt prior':>10}{'nxt net':>9}{'discipl':>9}  option"
    )
    for k in ("end_turn", "freeze_end_turn", "end_turn_8", "roll", "roll_then_freeze"):
        if k not in wi:
            continue
        v = wi[k]
        pr = v["parts"]
        extra = f" (froze in {v['frozen_in']}/{v['samples']})" if k == "roll_then_freeze" else ""
        L.append(
            f"    {v['score']:8.2f} {v.get('gap_to_chosen', float('nan')):+7.2f}"
            f" {f"{v['gap_to_end_turn_8']:+9.2f}" if 'gap_to_end_turn_8' in v else ' ' * 9} {_fmt(pr.get('combat'), 8)}"
            f"{_fmt(pr.get('next_prior'), 10)}{_fmt(pr.get('next_net'), 9)}{_fmt(pr.get('discipline'), 9)}"
            f"  {LABELS[k]}{extra}"
        )
    return "\n".join(L)


# ---------------------------------------------------------------------------------------------
# Value network
# ---------------------------------------------------------------------------------------------


def run_activations(
    bot: SovereignBot, trace: DecisionTrace, frame: int, npz: Dict[str, np.ndarray]
) -> Tuple[str, Dict[str, Any]]:
    pred = bot.evaluator.predictor
    if pred is None or not getattr(pred, "is_ready", False) or not hasattr(pred, "explain"):
        return "  Value net: not available for this bot", {}
    cands = [p for p in trace.plans if p.terminal_obs is not None]
    chosen = next((p for p in cands if p.chosen), None)
    others = [p for p in cands if p is not chosen]
    picks: List[Tuple[str, PlanTrace]] = []
    if chosen is not None:
        picks.append(("chosen", chosen))
    if others:
        picks.append(("other", max(others, key=lambda p: p.score)))
    if not picks:
        return "  Value net: no scored line with a terminal state at this decision", {}
    L = ["  Value net (end-of-turn view of each line's final state):"]
    data: Dict[str, Any] = {}
    for tag, plan in picks:
        ex = pred.explain(plan.terminal_obs, trace.ctx, end_of_turn=True)
        if not ex.get("ready"):
            continue
        probs = ex["placement_probs"]
        L.append(f"    [{tag}] {describe_line(trace.game, plan.actions)}")
        L.append(f"      utility {ex['utility']:.3f}  growth {ex['growth']:.3f}")
        L.append("      placement 1..8: " + " ".join(f"{x:.3f}" for x in probs))
        toks = [t for t in ex["tokens"] if t["zone"] != "global"]
        pos = sorted((t for t in toks if t["contribution"] > 0), key=lambda t: -t["contribution"])[:5]
        neg = sorted((t for t in toks if t["contribution"] < 0), key=lambda t: t["contribution"])[:5]
        L.append(
            "      helps: " + (", ".join(f"{t['name']} [{t['zone']}] {t['contribution']:+.2f}" for t in pos) or "-")
        )
        L.append(
            "      hurts: " + (", ".join(f"{t['name']} [{t['zone']}] {t['contribution']:+.2f}" for t in neg) or "-")
        )
        for k, arr in ex["capture"].items():
            npz[f"f{frame}_{tag}_{k}"] = arr
        data[tag] = {
            "line": [list(a) for a in plan.actions],
            "utility": ex["utility"],
            "growth": ex["growth"],
            "placement_probs": probs,
            "tokens": ex["tokens"],
        }
    return "\n".join(L), data


# ---------------------------------------------------------------------------------------------
# Driver
# ---------------------------------------------------------------------------------------------


def _num_records(lobby: seaglass.LobbyGame) -> int:
    n = lobby.num_records
    return int(n() if callable(n) else n)


class Explainer:
    def __init__(self, args: argparse.Namespace, out=None) -> None:
        self.args = args
        self.out = out or sys.stdout
        self.results: List[Dict[str, Any]] = []
        self.npz: Dict[str, np.ndarray] = {}
        self.mismatch: Optional[str] = None

    def say(self, text: str = "") -> None:
        print(text, file=self.out)
        self.out.flush()

    def run(self) -> int:
        a = self.args
        tl = seaglass.Timeline.load(a.replay)
        header = tl.header
        names = [p["name"] for p in header["players"]]
        player = resolve_player(a.player, names)
        lobby = build_lobby(header)
        seed, src = infer_bot_seed(header, player, a.bot_seed)
        bot = SovereignBot(seed=seed, weights_path=a.weights)
        decisions = tl.decisions()
        mine = [d for d in decisions if d[1] == player]
        if a.frame is not None:
            if not any(d[0] == a.frame for d in mine):
                raise ExplainError(f"frame {a.frame} is not a decision of {names[player]}")
            stop_frame = a.frame
        else:
            stop_frame = max((d[0] for d in mine if tl[d[0]].turn == a.turn), default=None)
            if stop_frame is None:
                raise ExplainError(f"{names[player]} has no decisions on turn {a.turn}")
        self.say(
            f"Replay {a.replay}: re-running {names[player]} (bot seed {seed}, from {src}) "
            f"up to frame {stop_frame}; lobby rebuilt with defaults except players/seed/names/turn_timer=False"
        )

        phase_open = False
        skip_forced = 0
        for frame, p, rec_act, note in decisions:
            if frame > stop_frame:
                break
            if skip_forced > 0 and p == self.skip_player:
                skip_forced -= 1
                continue
            if not phase_open:
                lobby.begin_phase()
                phase_open = True
            if lobby.is_done(p):
                lobby.end_phase()
                if lobby.done:
                    self.mismatch = f"lobby finished before frame {frame}"
                    break
                lobby.begin_phase()
            if note and note.get("forced"):
                before = _num_records(lobby)
                lobby.force_end(p)
                skip_forced = max(0, _num_records(lobby) - before - 1)
                self.skip_player = p
                continue
            if p != player:
                lobby.step(p, rec_act)
                continue

            obs = lobby.observe(p)
            live = bot.act_in_lobby(lobby, p)
            trace = bot.last_trace
            assert trace is not None
            matches = _action_key(live) == _action_key(rec_act)
            trace.turn = obs.turn
            if self._selected(frame, obs.turn, trace):
                self._report(frame, p, obs, trace, bot, matches)
            if not matches:
                self.mismatch = (
                    f"MISMATCH at frame {frame} (turn {obs.turn}): live {describe_action(_action_key(live), obs)} "
                    f"[{index_form(_action_key(live))}] vs recorded {describe_action(_action_key(rec_act), obs)} "
                    f"[{index_form(_action_key(rec_act))}]"
                )
                self.say("!" * 100)
                self.say(self.mismatch)
                if not a.keep_going:
                    self.say("Stopping: later output would not be what the recorded game saw (use --keep-going).")
                    break
            lobby.step(p, rec_act)

        if self.mismatch is None and phase_open and not lobby.done:
            pass  # stopped at the last selected decision; nothing more to play
        if a.activations and self.npz:
            np.savez_compressed(a.activations, **self.npz)
            self.say(f"Saved {len(self.npz)} activation arrays to {a.activations}")
        if a.json:
            with open(a.json, "w") as fh:
                json.dump(
                    {
                        "replay": a.replay,
                        "player": player,
                        "bot_seed": seed,
                        "mismatch": self.mismatch,
                        "decisions": self.results,
                    },
                    fh,
                    indent=1,
                )
            self.say(f"Wrote {a.json}")
        return 1 if self.mismatch else 0

    skip_player = -1

    def _selected(self, frame: int, turn: int, trace: DecisionTrace) -> bool:
        a = self.args
        if a.frame is not None:
            return frame == a.frame
        if turn != a.turn:
            return False
        return a.all or trace.kind == "search" or trace.finalize_rule is not None

    def _report(
        self,
        frame: int,
        player: int,
        obs: seaglass.TavernObservation,
        trace: DecisionTrace,
        bot: SovereignBot,
        matches: bool,
    ) -> None:
        a = self.args
        self.say(format_decision(frame, player, obs, trace, matches))
        entry: Dict[str, Any] = {"frame": frame, "turn": obs.turn, "matches_recording": matches, "trace": trace.to_dict()}
        why = blocked_refresh(trace)
        want_wi = a.what_if and trace.game is not None and (why is not None or (a.all and trace.kind == "search"))
        if want_wi:
            wi = run_what_ifs(bot, trace)
            if wi:
                self.say(format_what_if(trace, wi, f"{why} blocked a legal Roll" if why else "requested with --all"))
                entry["what_if"] = {"blocked_by": why, **wi}
        if a.activations and trace.plans:
            text, data = run_activations(bot, trace, frame, self.npz)
            self.say(text)
            if data:
                entry["activations"] = data
        self.results.append(entry)


def build_parser() -> argparse.ArgumentParser:
    ap = argparse.ArgumentParser(
        prog="python -m seaglass_bots.explain",
        description=(
            "Re-run a SovereignBot seat through a recorded solo game and explain its decisions: the lines it "
            "compared, the gates that blocked options, optional what-if scores and value-net attributions. "
            "The lobby is rebuilt with LobbyGame(players=n, seed=shared_seed, names=..., turn_timer=False) and "
            "every other setting at its default; per-player seeds must be shared_seed + index (checked)."
        ),
    )
    ap.add_argument("replay", help="a .replay.pb written by LobbyGame.save / the arena")
    ap.add_argument("--player", required=True, help="seat index, 'p3' (= index 3) or the player's name")
    sel = ap.add_mutually_exclusive_group(required=True)
    sel.add_argument("--turn", type=int, help="explain the seat's search decisions on this turn")
    sel.add_argument("--frame", type=int, help="explain the single decision at this timeline frame")
    ap.add_argument("--bot-seed", type=int, default=None, help="the bot's constructor seed (default: replay meta, else arena convention)")
    ap.add_argument("--weights", default=None, help="value-net .npz (default: the bot's built-in weights)")
    ap.add_argument("--all", action="store_true", help="every decision of the turn, not just searches / finalizes; what-ifs at every search")
    ap.add_argument("--what-if", action="store_true", help="score end turn / freeze / roll / roll+freeze where a gate blocked a Roll")
    ap.add_argument("--activations", metavar="OUT.npz", help="value-net attributions for the chosen and best other line; save raw arrays")
    ap.add_argument("--json", metavar="OUT.json", help="write the traces (and what-ifs) as JSON")
    ap.add_argument("--keep-going", action="store_true", help="continue after the bot's action differs from the recording")
    return ap


def main(argv: Optional[Sequence[str]] = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return Explainer(args).run()
    except ExplainError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
