"""Decision tracing for `SovereignBot`: what the search compared, which hand-written gates blocked options.

Everything here only *records* values the bot already computed; tracing never draws from the bot's RNGs.
"""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Sequence, Tuple

import seaglass

ActionKey = Tuple[str, Optional[int], Optional[int]]

# Breakdown keys `SovereignEvaluator.evaluate_turn_end(..., parts=)` fills.
PARTS_KEYS = (
    "combat_icm",
    "offense",
    "cvar_penalty",
    "boss_bonus",
    "combat",
    "survival_prob",
    "continuation_weight",
    "next_prior",
    "next_net",
    "discipline",
    "macro",
    "total",
)

MAX_CANDIDATES = 6
# Set False (or env SEAGLASS_TRACE_CANDIDATES=0) to keep dict candidates out of notes, for engines that
# only accept `(TavernAction, prob)` pairs.
EMIT_CANDIDATES = os.environ.get("SEAGLASS_TRACE_CANDIDATES", "1") != "0"
FREEZE_THRESHOLD = 1.5


def _jsonable(v: Any) -> Any:
    if isinstance(v, dict):
        return {str(k): _jsonable(x) for k, x in v.items()}
    if isinstance(v, (list, tuple)):
        return [_jsonable(x) for x in v]
    if isinstance(v, bool) or v is None or isinstance(v, (int, str)):
        return v
    try:
        return float(v)
    except (TypeError, ValueError):
        return str(v)


@dataclass
class PlanTrace:
    label: str
    actions: List[ActionKey]
    score: float
    fast: Optional[float] = None
    fast_parts: Dict[str, float] = field(default_factory=dict)
    parts: Dict[str, float] = field(default_factory=dict)
    samples: int = 1
    chosen: bool = False
    terminal_obs: Any = None  # not serialized

    def to_dict(self) -> Dict[str, Any]:
        return {
            "label": self.label,
            "actions": [list(a) for a in self.actions],
            "score": float(self.score),
            "fast": None if self.fast is None else float(self.fast),
            "fast_parts": _jsonable(self.fast_parts),
            "parts": _jsonable(self.parts),
            "samples": int(self.samples),
            "chosen": bool(self.chosen),
        }


@dataclass
class GateTrace:
    name: str
    passed: bool
    detail: str
    values: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        return {
            "name": self.name,
            "passed": bool(self.passed),
            "detail": self.detail,
            "values": _jsonable(self.values),
        }


@dataclass
class DecisionTrace:
    player: int
    turn: int
    kind: str = "search"
    chosen: Optional[ActionKey] = None
    value: Optional[float] = None
    epoch_ev: Optional[float] = None
    margin: float = 0.22
    plans: List[PlanTrace] = field(default_factory=list)
    gates: List[GateTrace] = field(default_factory=list)
    finalize_rule: Optional[str] = None
    queued: Optional[Dict[str, Any]] = None
    counts: Dict[str, int] = field(default_factory=dict)
    # Not serialized: what the decision was made on, for what-ifs in explain.py (references only).
    game: Any = None
    particles: Any = None
    ctx: Any = None
    legal: Any = None

    def chosen_plan(self) -> Optional[PlanTrace]:
        for p in self.plans:
            if p.chosen:
                return p
        return None

    def gate(self, name: str) -> Optional[GateTrace]:
        for g in self.gates:
            if g.name == name:
                return g
        return None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "player": int(self.player),
            "turn": int(self.turn),
            "kind": self.kind,
            "chosen": None if self.chosen is None else list(self.chosen),
            "value": None if self.value is None else float(self.value),
            "epoch_ev": None if self.epoch_ev is None else float(self.epoch_ev),
            "margin": float(self.margin),
            "plans": [p.to_dict() for p in self.plans],
            "gates": [g.to_dict() for g in self.gates],
            "finalize_rule": self.finalize_rule,
            "queued": _jsonable(self.queued),
            "counts": _jsonable(self.counts),
        }


# ---------------------------------------------------------------------------------------------
# ActionKey <-> TavernAction
# ---------------------------------------------------------------------------------------------

_ZERO_ARG = {
    "refresh": seaglass.TavernAction.refresh,
    "upgrade_tavern": seaglass.TavernAction.upgrade_tavern,
    "toggle_freeze": seaglass.TavernAction.toggle_freeze,
    "end_turn": seaglass.TavernAction.end_turn,
    "dark_discovery": seaglass.TavernAction.dark_discovery,
}
_ONE_ARG = {
    "buy": seaglass.TavernAction.buy,
    "sell": seaglass.TavernAction.sell,
    "choose_discover": seaglass.TavernAction.choose_discover,
    "choose_hero": seaglass.TavernAction.choose_hero,
    "choose_trinket": seaglass.TavernAction.choose_trinket,
    "pass": seaglass.TavernAction.pass_card,
}
_TWO_ARG = {
    "play": seaglass.TavernAction.play,
    "reposition": seaglass.TavernAction.reposition,
    "play_on_shop": seaglass.TavernAction.play_on_shop,
}


def key_to_action(key: Sequence[Any]) -> seaglass.TavernAction:
    """Inverse of `sovereign_search._action_key` (`(kind, arg0, arg1)` -> `TavernAction`)."""
    kind, a0, a1 = key
    if kind in _ZERO_ARG:
        return _ZERO_ARG[kind]()
    if kind in _ONE_ARG:
        return _ONE_ARG[kind](a0)
    if kind in _TWO_ARG:
        return _TWO_ARG[kind](a0, a1)
    if kind == "activate":  # (board_pos, optional target_pos)
        return seaglass.TavernAction.activate(a0, a1)
    if kind == "hero_power":  # optional target
        return seaglass.TavernAction.hero_power(a0)
    if kind == "hero_power_2":
        return seaglass.TavernAction.hero_power_2(a0)
    raise ValueError(f"unknown action kind {kind!r}")


# ---------------------------------------------------------------------------------------------
# The note written to the replay
# ---------------------------------------------------------------------------------------------


def _r(x: float) -> float:
    return round(float(x), 3)


def build_candidates(trace: DecisionTrace) -> List[Dict[str, Any]]:
    """At most `MAX_CANDIDATES` dicts, best first; one per distinct first action; always the chosen line."""
    chosen = trace.chosen_plan()
    best_by_first: Dict[ActionKey, PlanTrace] = {}
    for p in trace.plans:
        if not p.actions:
            continue
        first = tuple(p.actions[0])
        cur = best_by_first.get(first)
        if cur is None or (not cur.chosen and (p.chosen or p.score > cur.score)):
            best_by_first[first] = p
    lines = sorted(best_by_first.values(), key=lambda p: p.score, reverse=True)
    if len(lines) > MAX_CANDIDATES:
        top = lines[:MAX_CANDIDATES]
        if chosen is not None and chosen not in top:
            top = top[: MAX_CANDIDATES - 1] + [chosen]
            top.sort(key=lambda p: p.score, reverse=True)
        lines = top
    out: List[Dict[str, Any]] = []
    for p in lines:
        try:
            action = key_to_action(p.actions[0])
            plan = [key_to_action(k) for k in p.actions[1:]]
        except Exception:
            continue
        out.append({"action": action, "score": _r(p.score), "plan": plan, "label": p.label})
    return out


def note_from_trace(trace: Optional[DecisionTrace], legacy_note: Optional[Dict[str, float]]) -> Optional[Dict[str, Any]]:
    """The replay note: today's keys (`legacy_note`) plus `candidates` and numeric extras from the trace."""
    if trace is None:
        return legacy_note
    note: Dict[str, Any] = dict(legacy_note) if legacy_note else {}
    if trace.value is not None and "value" not in note:
        note["value"] = _r(trace.value)
    if trace.epoch_ev is not None and "epoch_ev" not in note:
        note["epoch_ev"] = _r(trace.epoch_ev)

    if trace.kind == "search":
        cands = build_candidates(trace) if EMIT_CANDIDATES else []
        if cands:
            note["candidates"] = cands
        cp = trace.chosen_plan()
        if cp is not None and cp.parts:
            for key, src in (
                ("combat", "combat"),
                ("macro_prior", "next_prior"),
                ("macro_net", "next_net"),
                ("discipline", "discipline"),
            ):
                if src in cp.parts:
                    note[key] = _r(cp.parts[src])
        g = trace.gate("root_refresh")
        if g is not None and not g.passed and g.values.get("legal"):
            note["roll_gated"] = 1.0
            note["roll_gold_needed"] = float(g.values.get("gold_needed", 0))
    if trace.queued is not None and trace.queued.get("plan_value") is not None:
        note["plan_value"] = _r(trace.queued["plan_value"])
    fg = trace.gate("smart_freeze")
    if fg is not None and fg.values.get("freeze_value") is not None:
        note["freeze_value"] = _r(fg.values["freeze_value"])
    return note if note else None
