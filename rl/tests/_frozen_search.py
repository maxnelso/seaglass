"""A frozen copy of the pre-batching `search_decision` (reference for the equivalence tests)."""

from __future__ import annotations

from typing import Any

import numpy as np
import seaglass
import torch

from seaglass_rl import features
from seaglass_rl.actions import ACTION_SPACE_SIZE, END_TURN_INDEX, action_from_index, action_mask
from seaglass_rl.features import encode_observation
from seaglass_rl.play import PlayOptions
from seaglass_rl.search import (
    SearchResult, SearchSpec, _Line, _end_enc, _mix, _sample, _sigma, completed_policy, line_budget, shopping_flags,
)


def _forward(model: Any, encs: list[tuple], masks: list[np.ndarray]) -> tuple[torch.Tensor, torch.Tensor]:
    pv = torch.from_numpy(np.stack([e[0] for e in encs]))
    ci = torch.from_numpy(np.stack([e[1] for e in encs]))
    nf = torch.from_numpy(np.stack([e[2] for e in encs]))
    mk = torch.from_numpy(np.stack(masks))
    with torch.inference_mode():
        logits, values = model(pv, ci, nf, mk)
    return logits, values


def search_decision(
    model: Any,
    game: seaglass.LobbyGame,
    p: int,
    opts: PlayOptions,
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




def _run_lines(
    model: Any,
    lines: list[_Line],
    ctx: Any,
    rust: Any,
    opts: PlayOptions,
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
