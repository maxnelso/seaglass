"""Decision tracing for SovereignBot: traces, notes, predictor capture, and the explain CLI."""

from __future__ import annotations

import json
import os

import numpy as np
import pytest
import seaglass

from seaglass_bots import explain as explain_mod
from seaglass_bots import sovereign_trace as st
from seaglass_bots.arena import _seats_meta
from seaglass_bots.baseline import SemiRandomBot
from seaglass_bots.sovereign_search import SovereignBot, _action_key
from seaglass_bots.sovereign_trace import DecisionTrace, GateTrace, PlanTrace, key_to_action, note_from_trace

BOT_SEED = 1234
GAME_SEED = 77
MAX_TURNS = 4


class _Game:
    pass


@pytest.fixture(scope="module")
def game(tmp_path_factory):
    """One short 8-player game: Sovereign in seat 0, SemiRandom elsewhere. Notes are passed to `step`
    (without dict candidates, which only a rebuilt engine accepts)."""
    old = st.EMIT_CANDIDATES
    st.EMIT_CANDIDATES = False
    try:
        names = [f"Search(sovereign)_p0"] + [f"SemiRandom_p{i}" for i in range(1, 8)]
        lobby = seaglass.LobbyGame(players=8, seed=GAME_SEED, names=names, max_turns=MAX_TURNS)
        bots = {0: SovereignBot(seed=BOT_SEED)}
        for p in range(1, 8):
            bots[p] = SemiRandomBot(seed=GAME_SEED + 100 + p)
        g = _Game()
        g.records = []  # (trace, note, action, legal)
        while not lobby.done:
            lobby.begin_phase()
            for p in list(lobby.active_players()):
                while not lobby.is_done(p):
                    legal = lobby.legal_actions(p)
                    act = bots[p].act_in_lobby(lobby, p)
                    if not lobby.is_legal(p, act):
                        lobby.force_end(p)
                        break
                    note = getattr(bots[p], "last_note", None)
                    if p == 0:
                        g.records.append((bots[0].last_trace, note, act, legal))
                    lobby.step(p, act, note=note)
                if not lobby.is_done(p):
                    lobby.force_end(p)
            lobby.end_phase()
        lobby.set_meta("seats", _seats_meta({p: n.split("_")[0] for p, n in enumerate(names)}, {0: BOT_SEED}))
        g.path = str(tmp_path_factory.mktemp("replay") / "g.replay.pb")
        lobby.save(g.path)
        g.bot = bots[0]
        yield g
    finally:
        st.EMIT_CANDIDATES = old


def _searches(game):
    return [r for r in game.records if r[0].kind == "search"]


def test_traces_filled(game):
    kinds = {r[0].kind for r in game.records}
    assert "search" in kinds and "hero" in kinds
    assert len({id(r[0]) for r in game.records}) == len(game.records)  # a fresh trace per decision
    for trace, note, act, _ in game.records:
        assert trace.chosen == _action_key(act)
        assert trace.kind in {"only_legal", "hero", "duos_pass", "trinket", "discover", "queued", "search"}
        if trace.kind == "search":
            assert trace.plans and trace.value is not None and trace.epoch_ev is not None
            assert sum(p.chosen for p in trace.plans) <= 1
            assert trace.chosen_plan() is not None or trace.finalize_rule is not None
            assert trace.game is not None and trace.particles is not None
            labels = {p.label for p in trace.plans}
            assert "plan" in labels
            for p in trace.plans:
                assert np.isfinite(p.score) and p.actions
                if p.label == "plan":
                    assert p.fast is not None and "prior" in p.fast_parts and "net" in p.fast_parts
                    assert p.terminal_obs is not None
                for k in ("combat", "macro", "total", "next_prior", "next_net", "discipline", "survival_prob"):
                    assert k in p.parts, (p.label, k)
                assert abs(p.parts["total"] - p.score) < 1e-4 or p.samples > 1 or p.label == "plan"
            assert trace.counts["terminals"] > 0
            json.dumps(trace.to_dict())
        if trace.kind == "queued":
            assert trace.queued and "steps_left" in trace.queued
    assert len(_searches(game)) >= 2


def test_plan_parts_add_up(game):
    for trace, *_ in _searches(game)[:3]:
        for p in trace.plans:
            if p.label == "plan":
                assert p.parts["total"] == pytest.approx(p.score, abs=1e-9)
                assert p.parts["combat"] + p.parts["macro"] == pytest.approx(p.parts["total"], abs=1e-6)
                assert p.fast_parts["prior"] + p.fast_parts["net"] == pytest.approx(p.fast, abs=1e-6)


def test_fresh_note_per_decision(game):
    """A forced / single-legal action no longer repeats the previous decision's note."""
    for trace, note, _, _ in game.records:
        if trace.kind == "only_legal":
            assert not note


def test_note_values_are_floats_and_legacy_keys_kept(game):
    for trace, note, *_ in game.records:
        for k, v in (note or {}).items():
            assert k != "candidates"  # disabled in the fixture
            assert isinstance(v, float), (k, v)
        if trace.kind == "search":
            assert "value" in note and "epoch_ev" in note


def test_note_from_trace_shape(game):
    searches = _searches(game)
    trace, legacy = searches[0][0], searches[0][1]
    old = st.EMIT_CANDIDATES
    st.EMIT_CANDIDATES = True
    try:
        note = note_from_trace(trace, legacy)
    finally:
        st.EMIT_CANDIDATES = old
    cands = note["candidates"]
    assert 1 <= len(cands) <= 6
    assert [c["score"] for c in cands] == sorted((c["score"] for c in cands), reverse=True)
    firsts = [_action_key(c["action"]) for c in cands]
    assert len(set(firsts)) == len(firsts)  # one per distinct first action
    for c in cands:
        assert set(c) == {"action", "score", "plan", "label"}
        assert isinstance(c["action"], seaglass.TavernAction)
        assert all(isinstance(a, seaglass.TavernAction) for a in c["plan"])
    cp = trace.chosen_plan()
    if cp is not None:  # the chosen line is always listed, with its own plan
        mine = [c for c in cands if _action_key(c["action"]) == tuple(cp.actions[0])][0]
        assert [_action_key(a) for a in mine["plan"]] == [tuple(k) for k in cp.actions[1:]]
        assert "combat" in note
    for k, v in legacy.items():  # legacy keys untouched
        assert note[k] == v


def test_note_from_trace_pure_python():
    """The builder on a hand-made trace: dedup per first action, chosen line kept, cap at 6, extras."""
    keys = [("buy", i, None) for i in range(8)]
    t = DecisionTrace(player=0, turn=3, kind="search", value=5.0, epoch_ev=4.0)
    for i, k in enumerate(keys):
        t.plans.append(PlanTrace("plan", [k, ("end_turn", None, None)], score=10.0 - i, parts={"combat": 1.0 + i}))
    t.plans.append(PlanTrace("plan", [keys[0], ("sell", 0, None), ("end_turn", None, None)], score=9.5))  # worse dup
    t.plans.append(PlanTrace("roll", [("refresh", None, None)], score=-3.0, chosen=True, parts={
        "combat": 2.0, "next_prior": 3.0, "next_net": 0.5, "discipline": -1.0}))
    t.gates.append(GateTrace("root_refresh", False, "gold 1 < 3", {"legal": True, "gold_needed": 3}))
    t.gates.append(GateTrace("smart_freeze", False, "x", {"freeze_value": -2.0}))
    t.queued = {"plan_value": 7.123456, "steps_left": 2, "label": "plan"}
    old = st.EMIT_CANDIDATES
    st.EMIT_CANDIDATES = True
    try:
        note = note_from_trace(t, {"value": 5.0})
    finally:
        st.EMIT_CANDIDATES = old
    cands = note["candidates"]
    assert len(cands) == 6
    assert any(c["label"] == "roll" for c in cands)  # the chosen line survives the cap, though it scores lowest
    assert [c["score"] for c in cands] == sorted((c["score"] for c in cands), reverse=True)
    assert note["roll_gated"] == 1.0 and note["roll_gold_needed"] == 3.0
    assert note["freeze_value"] == -2.0 and note["plan_value"] == 7.123
    assert note["combat"] == 2.0 and note["macro_prior"] == 3.0 and note["macro_net"] == 0.5 and note["discipline"] == -1.0
    assert note_from_trace(None, {"a": 1.0}) == {"a": 1.0}
    # without candidates
    st.EMIT_CANDIDATES = False
    try:
        assert "candidates" not in note_from_trace(t, None)
    finally:
        st.EMIT_CANDIDATES = old


def test_key_to_action_roundtrip(game):
    seen = set()
    for trace, note, act, legal in game.records:
        for a in legal:
            k = _action_key(a)
            b = key_to_action(k)
            assert _action_key(b) == k and b.kind == a.kind
            seen.add(a.kind)
    assert {"buy", "end_turn", "play"} <= seen
    # kinds the short game may not offer
    for key in [
        ("sell", 2, None), ("reposition", 1, 3), ("activate", 0, 2), ("activate", 1, None), ("refresh", None, None),
        ("upgrade_tavern", None, None), ("toggle_freeze", None, None), ("choose_discover", 1, None),
        ("play_on_shop", 1, 2), ("pass", 2, None), ("hero_power", None, None), ("hero_power", 3, None),
        ("hero_power_2", None, None), ("choose_hero", 1, None), ("choose_trinket", 0, None),
        ("dark_discovery", None, None), ("buy", 4, None), ("play", 0, 5), ("end_turn", None, None),
    ]:
        assert _action_key(key_to_action(key)) == key
    with pytest.raises(ValueError):
        key_to_action(("nope", None, None))


def test_predictor_capture_exact(game):
    pred = game.bot.evaluator.predictor
    assert pred is not None and pred.is_ready
    obss = [r[0].plans[0].terminal_obs for r in _searches(game)[:4]]
    obss = [o for o in obss if o is not None]
    ctx = _searches(game)[0][0].ctx
    vocab, slot, pos, num, mask, glob = pred.featurizer.featurize_batch(obss, ctx, True)
    plain = pred.forward_arrays(vocab, slot, pos, num, mask, glob)
    cap: dict = {}
    with_cap = pred.forward_arrays(vocab, slot, pos, num, mask, glob, capture=cap)
    assert np.array_equal(plain, with_cap)
    assert plain.tolist() == pred.predict_batch(obss, ctx=ctx, end_of_turn=True)
    b, n_tok = vocab.shape
    assert cap["attn_0"].shape == (b, pred.n_heads, n_tok, n_tok)
    assert cap["placement_probs"].shape == (b, 8)
    assert np.allclose(cap["placement_probs"].sum(-1), 1.0, atol=1e-5)
    assert cap["tokens_in"].shape[:2] == (b, n_tok) and cap["readout"].shape[0] == b
    assert np.array_equal(cap["utility"], plain)
    for k in ("vocab", "slot", "pos", "mask", "trunk", "growth", "tokens_0"):
        assert k in cap


def test_predictor_explain(game):
    pred = game.bot.evaluator.predictor
    trace = _searches(game)[0][0]
    obs = trace.plans[0].terminal_obs
    ex = pred.explain(obs, trace.ctx, end_of_turn=True)
    assert ex["ready"]
    assert ex["utility"] == pytest.approx(pred.predict_batch([obs], ctx=trace.ctx, end_of_turn=True)[0], abs=1e-4)
    assert len(ex["placement_probs"]) == 8
    zones = {t["zone"] for t in ex["tokens"]}
    assert "global" in zones and "board" in zones
    n_board = len(obs.board)
    assert sum(t["zone"] == "board" for t in ex["tokens"]) == min(7, n_board)
    glob = [t for t in ex["tokens"] if t["zone"] == "global"][0]
    assert glob["contribution"] == 0.0
    assert any(t["contribution"] != 0.0 for t in ex["tokens"] if t["zone"] == "board")
    assert all(isinstance(t["name"], str) and t["name"] for t in ex["tokens"])


def test_freeze_value_matches_should_freeze(game):
    ev = game.bot.evaluator
    for trace, *_ in _searches(game)[:4]:
        obs = trace.game.observe()
        assert ev.should_freeze_current_shop(obs, ctx=trace.ctx) == (
            ev.freeze_shop_value(obs, ctx=trace.ctx) >= 1.5
        )


def test_finalize_rule_and_gates(game):
    finals = [r[0] for r in game.records if r[0].finalize_rule is not None]
    assert finals, "a four-turn game ends turns through the finalize path"
    for t in finals:
        assert t.finalize_rule in {"rescue_open_play", "smart_freeze", "zero_waste_refresh", "positioning", "end_turn"}
        for g in t.gates:
            assert g.name and isinstance(g.detail, str) and g.detail
    assert any(t.gate("smart_freeze") is not None for t in finals)


def test_arena_seats_meta():
    s = json.loads(_seats_meta({0: "Search(sovereign)", 1: "Tempo"}, {0: 5}))
    assert s == [{"seat": 0, "bot": "Search(sovereign)", "seed": 5}, {"seat": 1, "bot": "Tempo", "seed": None}]


def test_explain_runs_without_divergence(game, tmp_path, capsys):
    npz = tmp_path / "a.npz"
    js = tmp_path / "a.json"
    rc = explain_mod.main(
        [game.path, "--player", "p0", "--turn", "3", "--what-if", "--activations", str(npz), "--json", str(js)]
    )
    out = capsys.readouterr().out
    assert rc == 0, out
    assert "MISMATCH" not in out
    assert "Decision: search" in out and "score" in out and "Gates:" in out
    data = json.loads(js.read_text())
    assert data["mismatch"] is None and data["decisions"]
    # the explained search values equal the live run's
    live_turn3 = [r[0] for r in game.records if r[0].kind == "search" and r[0].turn == 3]
    got = [d["trace"]["value"] for d in data["decisions"] if d["trace"]["kind"] == "search"]
    assert got == pytest.approx([t.value for t in live_turn3], abs=1e-9)
    arrs = np.load(npz)
    assert any(k.endswith("_attn_0") for k in arrs.files)


def test_explain_frame_and_errors(game, capsys):
    tl = seaglass.Timeline.load(game.path)
    mine = [d for d in tl.decisions() if d[1] == 0]
    frame = mine[len(mine) // 2][0]
    assert explain_mod.main([game.path, "--player", "0", "--frame", str(frame)]) == 0
    assert f"Frame {frame}" in capsys.readouterr().out
    # a wrong bot seed diverges (or at least reports), and stops
    rc = explain_mod.main([game.path, "--player", "Search(sovereign)_p0", "--turn", "4", "--bot-seed", "99999"])
    out = capsys.readouterr().out
    assert rc in (0, 1)
    assert explain_mod.main([game.path, "--player", "p9", "--turn", "3"]) == 2


def test_engine_accepts_candidate_notes(game):
    """Needs an engine build that takes dict candidates in notes; skipped on older builds."""
    trace, legacy, act, _ = _searches(game)[0]
    old = st.EMIT_CANDIDATES
    st.EMIT_CANDIDATES = True
    try:
        note = note_from_trace(trace, legacy)
    finally:
        st.EMIT_CANDIDATES = old
    lobby = seaglass.LobbyGame(players=2, seed=1, names=["a", "b"])
    lobby.begin_phase()
    try:
        lobby.step(0, lobby.legal_actions(0)[0], note=note)
    except TypeError as e:
        pytest.skip(f"installed engine rejects dict candidates: {e}")
