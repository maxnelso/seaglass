"""Policy-guided search inside a player's turn, scored by the learned placement critic.

The world inside a turn is exact except for hidden randomness (refreshes, Discovers). So at a
decision of the shopping step, the search tries a few root actions, and for each one plays out the
rest of the turn with the policy itself. A *line* is that rest of the turn: the root action, then
the policy's own actions (sampled at `SearchSpec.temperature`, under the move rules of `play_games`),
until the policy picks End Turn (or `max_line` actions have passed). The line is scored by the
critic's value of the end-of-turn state (`arranging=True`, the state the critic sees at a turn's last
decision in training), before End Turn is applied. No combat, no future turns.

Every line runs in its own belief Tavern (`LobbyGame.belief_tavern`): the player's own state with a
belief-adjusted card pool, so the search never sees the true hidden pool or the true future shop.
The live game is never touched.

The root action is chosen by Gumbel MuZero's sequential halving (Danihelka et al., 2022): the top
`top_m` actions by Gumbel noise plus policy logit are the candidates; in each phase, every remaining
candidate gets some lines (the same belief seeds for every candidate: common random numbers) and the
better half, by `g + logit + sigma(Q)`, goes on. `Q` is a candidate's mean line score, on [-1, 1].
The last one standing is played. The search also returns the improved policy
`softmax(logit + sigma(completed Q))` (the target for distillation) and the root value.

All the lines of a phase run in lockstep: one batched forward per step over the lines still going.
No hand-written play rules and no card knowledge: the policy proposes, the critic judges.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import TYPE_CHECKING, Any

import numpy as np
import seaglass
import torch

from . import features
from .actions import ACTION_SPACE_SIZE, END_TURN_INDEX, action_from_index, action_mask
from .features import encode_observation

if TYPE_CHECKING:
    from .play import PlayOptions

NEG = -1e9
_MASK64 = (1 << 64) - 1


@dataclass(frozen=True)
class SearchSpec:
    """The search's knobs."""

    # The root actions tried: the top `top_m` of Gumbel noise + policy logit.
    top_m: int = 8
    # Lines per decision in all, split over the halving phases and the candidates left in each (at
    # least 1 line per candidate per phase).
    sims: int = 16
    # The policy's temperature inside lines; 0 is greedy.
    temperature: float = 1.0
    # Gumbel noise on the root (for training targets); False: g = 0 (for evaluation).
    gumbel_noise: bool = False
    c_visit: float = 50.0
    c_scale: float = 1.0
    # Actions in a line before it is scored anyway.
    max_line: int = 40
    seed: int = 0

    def __post_init__(self) -> None:
        if self.top_m < 1 or self.sims < 1 or self.max_line < 1:
            raise ValueError("top_m, sims and max_line must be at least 1")
        if self.temperature < 0:
            raise ValueError("temperature must be >= 0")


@dataclass
class SearchResult:
    action: int
    # The improved policy over the legal actions (0 elsewhere).
    policy_target: np.ndarray
    # The critic's value of the root state.
    root_value: float
    # The candidates' mean line scores and line counts.
    q: dict[int, float]
    visits: dict[int, int]
    # Lines run, and batched forward calls made (the root's included).
    lines: int
    forwards: int
    # The policy's log-probability of `action` at the root (temperature 1, as `act_batch` gives it).
    log_prob: float = 0.0
    # The mixed value `v_mix` of the completed Q.
    v_mix: float = 0.0


def shopping_flags(opts: "PlayOptions") -> tuple[bool, bool]:
    """`(allow_reposition, allow_freeze)` of a model's shopping step under `opts` (the same rules as
    `play_games`'s `mask_for` outside the end-of-turn step)."""
    if opts.positioning_phase:
        return False, False
    return opts.allow_reposition, opts.allow_freeze


def line_budget(spec: SearchSpec, candidates: int) -> list[tuple[int, int]]:
    """The halving phases for `candidates` root actions: `(candidates left, lines each)` per phase.

    There are `ceil(log2(candidates))` phases, each with the candidates left (halved, rounded up, after
    every phase). A phase gives each `max(1, sims // (phases * left))` lines. The lines run in all are
    at most `max(sims, sum of the candidates left over the phases)`: `sims` when it is big enough
    to give every candidate 1 line per phase."""
    out = []
    left = candidates
    phases = math.ceil(math.log2(candidates)) if candidates > 1 else 0
    for _ in range(phases):
        out.append((left, max(1, spec.sims // (phases * left))))
        left = (left + 1) // 2
    return out


def _mix(*xs: int) -> int:
    """A deterministic 64-bit hash of some integers (splitmix64 steps)."""
    h = 0x9E3779B97F4A7C15
    for x in xs:
        h = (h + (int(x) & _MASK64) + 0x9E3779B97F4A7C15) & _MASK64
        h = ((h ^ (h >> 30)) * 0xBF58476D1CE4E5B9) & _MASK64
        h = ((h ^ (h >> 27)) * 0x94D049BB133111EB) & _MASK64
        h ^= h >> 31
    return h


def decision_seed(*xs: int) -> int:
    """A deterministic seed from some integers (the game's seed, the player, the turn, the decision's
    index...)."""
    return _mix(*xs)


def _forward(model: Any, encs: list[tuple], masks: list[np.ndarray]) -> tuple[torch.Tensor, torch.Tensor]:
    pv = torch.from_numpy(np.stack([e[0] for e in encs]))
    ci = torch.from_numpy(np.stack([e[1] for e in encs]))
    nf = torch.from_numpy(np.stack([e[2] for e in encs]))
    mk = torch.from_numpy(np.stack(masks))
    with torch.inference_mode():
        logits, values = model(pv, ci, nf, mk)
    return logits, values


def _sample(logits: torch.Tensor, temperature: float, rng: np.random.Generator) -> np.ndarray:
    """One action per row of masked `logits`: the argmax at temperature 0, else a sample."""
    if temperature <= 0:
        return logits.argmax(dim=-1).numpy()
    probs = torch.softmax(logits.double() / temperature, dim=-1).numpy()
    cum = probs.cumsum(axis=-1)
    u = rng.random((len(probs), 1)) * cum[:, -1:]
    return np.minimum((cum < u).sum(axis=-1), probs.shape[1] - 1)


class _Line:
    __slots__ = ("cand", "tavern", "steps", "end_enc", "score")

    def __init__(self, cand: int, tavern: Any) -> None:
        self.cand = cand
        self.tavern = tavern
        self.steps = 0
        self.end_enc: tuple | None = None
        self.score = 0.0


def _sigma(q: np.ndarray | float, max_n: int, spec: SearchSpec) -> np.ndarray | float:
    """Gumbel MuZero's monotone transform of a Q on [-1, 1]."""
    return (spec.c_visit + max_n) * spec.c_scale * (q + 1.0) / 2.0


def search_decision(
    model: Any,
    game: seaglass.LobbyGame,
    p: int,
    opts: "PlayOptions",
    spec: SearchSpec,
    rng: np.random.Generator,
    seed: int,
    root: tuple[tuple, np.ndarray] | None = None,
) -> SearchResult:
    """Choose player `p`'s next action in the shopping step by search. `seed` identifies the decision
    (the game's seed, the player, the turn and the decision index mixed); the belief seeds derive from
    it and `spec.seed`. `rng` draws the Gumbel noise and the policy's samples inside lines. `root` is
    the root's `(encoding, mask)` if the caller has them (`play_games` does).

    The search is skipped, and the plain policy used (sampled at `spec.temperature`), if there are
    fewer than 2 legal actions or the lobby is not in a turn phase (the hero phase). The lines run
    at most `max(spec.sims, sum of candidates left per phase)`, see `line_budget`."""
    rust = features.rust_encoder() if opts.rust_encoder else None
    reposition, freeze = shopping_flags(opts)
    in_turn = game.phase == "turn"
    ctx = game.context(p) if in_turn else None
    if root is None:
        if rust is not None:
            enc = features.fast_encode(rust, game, p, False, in_turn)
            mask = features.fast_mask(rust, game, p, reposition, freeze, opts.prune_dominated)
        else:
            enc = encode_observation(game.observe(p), ctx, arranging=False)
            from .play import Seat

            mask = action_mask(
                Seat(game, p),
                allow_reposition=reposition,
                allow_freeze=freeze,
                prune_dominated=opts.prune_dominated,
            )
    else:
        enc, mask = root
    legal = np.flatnonzero(mask)
    forwards = 1
    logits_t, value_t = _forward(model, [enc], [mask])
    logits = logits_t[0].double().numpy()
    v0 = float(value_t[0])
    lse = float(logits[legal].max() + np.log(np.exp(logits[legal] - logits[legal].max()).sum())) if len(legal) else 0.0
    prior = np.zeros(ACTION_SPACE_SIZE)
    if len(legal):
        prior[legal] = np.exp(logits[legal] - lse)

    def result(action: int, target: np.ndarray, q: dict, n: dict, lines: int, vmix: float) -> SearchResult:
        return SearchResult(
            action=int(action),
            policy_target=target.astype(np.float32),
            root_value=v0,
            q=q,
            visits=n,
            lines=lines,
            forwards=forwards,
            log_prob=float(logits[action] - lse),
            v_mix=vmix,
        )

    if len(legal) < 2 or not in_turn:
        if not len(legal):
            return SearchResult(END_TURN_INDEX, prior.astype(np.float32), v0, {}, {}, 0, forwards)
        action = int(_sample(logits_t, spec.temperature, rng)[0])
        return result(action, prior, {}, {}, 0, v0)

    # The candidates: the top `top_m` legal actions by Gumbel noise + logit.
    g = rng.gumbel(size=ACTION_SPACE_SIZE) if spec.gumbel_noise else np.zeros(ACTION_SPACE_SIZE)
    base = g + logits
    order = legal[np.argsort(-base[legal], kind="stable")]
    cands = [int(a) for a in order[: spec.top_m]]
    q_sum = {a: 0.0 for a in cands}
    visits = {a: 0 for a in cands}
    lines_run = 0

    def mean_q(a: int) -> float:
        return q_sum[a] / visits[a] if visits[a] else 0.0

    alive = list(cands)
    for left, per in line_budget(spec, len(cands)):
        assert left == len(alive)
        lines: list[_Line] = []
        for a in alive:
            for i in range(per):
                j = visits[a] + i  # the same belief seed for every candidate's j-th line
                tavern = game.belief_tavern(p, seed=_mix(spec.seed, seed, j) & (2**63 - 1))
                line = _Line(a, tavern)
                if a == END_TURN_INDEX:
                    line.end_enc = _end_enc(rust, tavern, ctx)
                else:
                    act = action_from_index(a)
                    if tavern.is_legal(act):
                        tavern.step(act)
                    line.steps = 1
                lines.append(line)
        forwards += _run_lines(model, lines, ctx, rust, opts, spec, rng)
        # Score the finished lines with one forward.
        enc_end = [ln.end_enc for ln in lines]
        only_end = np.zeros(ACTION_SPACE_SIZE, dtype=np.bool_)
        only_end[END_TURN_INDEX] = True
        _, vals = _forward(model, enc_end, [only_end] * len(lines))
        forwards += 1
        for ln, v in zip(lines, vals.numpy()):
            q_sum[ln.cand] += float(v)
            visits[ln.cand] += 1
        lines_run += len(lines)
        max_n = max(visits.values())
        scores = {a: base[a] + float(_sigma(mean_q(a), max_n, spec)) for a in alive}
        alive.sort(key=lambda a: -scores[a])
        alive = alive[: (len(alive) + 1) // 2]

    action = alive[0]
    q = {a: mean_q(a) for a in cands if visits[a]}
    n = {a: visits[a] for a in cands if visits[a]}
    target, vmix = completed_policy(logits, legal, v0, q, n, spec)
    return result(action, target, q, n, lines_run, vmix)


def _end_enc(rust: Any, tavern: Any, ctx: Any) -> tuple:
    """The critic's view of a line's end: the end-of-turn observation (`arranging=True`)."""
    return features.fast_encode_tavern(rust, tavern, ctx, True)


def _run_lines(
    model: Any,
    lines: list[_Line],
    ctx: Any,
    rust: Any,
    opts: "PlayOptions",
    spec: SearchSpec,
    rng: np.random.Generator,
) -> int:
    """Play the lines to their ends in lockstep (one batched forward per step); each line's
    `end_enc` is set when it ends. Returns the forwards made."""
    reposition, freeze = shopping_flags(opts)
    forwards = 0
    running = [ln for ln in lines if ln.end_enc is None]
    while running:
        batch, encs, masks = [], [], []
        for ln in running:
            mask = features.fast_mask_tavern(rust, ln.tavern, reposition, freeze, opts.prune_dominated)
            if ln.steps >= spec.max_line or not mask.any():
                ln.end_enc = _end_enc(rust, ln.tavern, ctx)
                continue
            batch.append(ln)
            encs.append(features.fast_encode_tavern(rust, ln.tavern, ctx, False))
            masks.append(mask)
        if not batch:
            break
        logits, _ = _forward(model, encs, masks)
        forwards += 1
        acts = _sample(logits, spec.temperature, rng)
        running = []
        for ln, a in zip(batch, acts):
            a = int(a)
            if a == END_TURN_INDEX:
                ln.end_enc = _end_enc(rust, ln.tavern, ctx)
                continue
            try:
                done = ln.tavern.step(action_from_index(a))
            except ValueError:
                done = True
            ln.steps += 1
            if done:
                ln.end_enc = _end_enc(rust, ln.tavern, ctx)
            else:
                running.append(ln)
    return forwards


def completed_policy(
    logits: np.ndarray,
    legal: np.ndarray,
    root_value: float,
    q: dict[int, float],
    visits: dict[int, int],
    spec: SearchSpec,
) -> tuple[np.ndarray, float]:
    """The improved policy `softmax(logit + sigma(completed Q))` over the `legal` actions (0
    elsewhere), and `v_mix`. The completed Q is the candidates' mean line score, and `v_mix` for the
    other actions: the root value mixed with the visited actions' Q weighted by the prior (Gumbel
    MuZero, Eq. 33)."""
    out = np.zeros(ACTION_SPACE_SIZE)
    l = logits[legal].astype(np.float64)
    prior = np.exp(l - l.max())
    prior /= prior.sum()
    total = sum(visits.values())
    if total <= 0:
        out[legal] = prior
        return out, root_value
    pos = {int(a): k for k, a in enumerate(legal)}
    vis_p = sum(prior[pos[a]] for a in visits)
    vis_pq = sum(prior[pos[a]] * q[a] for a in visits)
    v_mix = (root_value + total * vis_pq / vis_p) / (1.0 + total) if vis_p > 1e-8 else root_value
    comp = np.full(len(legal), v_mix)
    for a, qa in q.items():
        comp[pos[a]] = qa
    max_n = max(visits.values())
    z = l + _sigma(comp, max_n, spec)
    z -= z.max()
    e = np.exp(z)
    out[legal] = e / e.sum()
    return out, float(v_mix)
