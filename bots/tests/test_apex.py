"""Unit, integration, and strict fair-play tests for ApexBot (`seaglass_bots` + `seaglass_rl`)."""

from __future__ import annotations

from pathlib import Path

import numpy as np
import seaglass
import torch

from seaglass_bots import (
    ApexBeliefTracker,
    ApexBot,
    ApexEvaluator,
    ApexPositioner,
    ApexValueNet,
    ApexValuePredictor,
)
from seaglass_bots.apex_knowledge import get_apex_knowledge
from seaglass_bots.apex_model import DEFAULT_APEX_NPZ_PATH
from seaglass_bots.apex_search import prune_apex_actions
from seaglass_rl.actions import action_from_index
from seaglass_rl.env import BattlegroundsEnv
from seaglass_rl.play import BOTS, LobbyConfig, PlayOptions, Player, Seat, GameSpec


class _StrictFairPlayLobbyProxy:
    """Proxy around `LobbyGame` that forbids `copy()` or any hidden opponent/pool inspection."""

    _ALLOWED = frozenset(
        {"observe", "legal_actions", "context", "belief_tavern", "phase", "duos", "turn"}
    )

    def __init__(self, lobby: seaglass.LobbyGame) -> None:
        object.__setattr__(self, "_lobby", lobby)

    def __getattr__(self, name: str):
        if name not in self._ALLOWED:
            raise AssertionError(f"Fair-play violation: attempted to access LobbyGame.{name}!")
        return getattr(object.__getattribute__(self, "_lobby"), name)


def test_apex_exports_and_rl_registration() -> None:
    assert "apex" in BOTS
    bot = ApexBot(beam_width=6, max_epoch_depth=4, top_k_combat_evals=2, refresh_samples=1)
    assert isinstance(bot.evaluator, ApexEvaluator)
    assert isinstance(bot.belief, ApexBeliefTracker)
    assert isinstance(bot.positioner, ApexPositioner)


def test_apex_solo_strict_fair_play_proxy() -> None:
    lobby = seaglass.LobbyGame(players=4, seed=4242)
    proxy = _StrictFairPlayLobbyProxy(lobby)
    bot = ApexBot(beam_width=6, max_epoch_depth=4, top_k_combat_evals=2, refresh_samples=1, seed=7)

    turns_completed = 0
    while not lobby.done and turns_completed < 4:
        lobby.begin_phase()
        is_turn = lobby.phase == "turn"
        for p in list(lobby.active_players()):
            steps = 0
            while not lobby.is_done(p) and steps < 40:
                if p == 0:
                    act = bot.act_in_lobby(proxy, p)  # type: ignore[arg-type]
                else:
                    legal = lobby.legal_actions(p)
                    act = legal[-1]
                assert lobby.is_legal(p, act), f"Illegal action {act} returned for player {p}"
                lobby.step(p, act)
                steps += 1
            if not lobby.is_done(p):
                lobby.force_end(p)
        lobby.end_phase()
        if is_turn:
            turns_completed += 1

    assert turns_completed >= 3


def test_apex_duos_lobby_and_play_integration() -> None:
    cfg = LobbyConfig(duos=True, players=4, max_turns=6)
    spec = GameSpec(seed=7070, controllers=["apex", "heuristic"])
    player = Player(0)
    try:
        results = player.play(cfg, [spec], {}, PlayOptions(), record=(), seed=7070)
    finally:
        player.close()
    assert len(results) == 1
    assert len(results[0].placements) == 2
    assert set(results[0].placements) == {1, 2}


def test_apex_rl_seat_env_and_tavern_rng_isolation() -> None:
    bot = ApexBot(beam_width=6, max_epoch_depth=4, top_k_combat_evals=2, refresh_samples=1, seed=19)

    # 1. `seaglass_rl.play.Seat`
    lobby = seaglass.LobbyGame(players=2, seed=111)
    lobby.begin_phase()
    seat = Seat(lobby, 0)
    idx_seat = bot.act(seat)
    assert seat.is_legal(action_from_index(idx_seat))

    # 2. `BattlegroundsEnv.player` (`seaglass.MatchPlayer`) — step a full episode action!
    env = BattlegroundsEnv(opponent="heuristic", max_turns=4)
    env.reset(seed=222)
    idx_mp = bot.act(env.player)
    assert env.player.is_legal(action_from_index(idx_mp))
    env.step(idx_mp)

    # 3. Standalone `seaglass.TavernGame` — verify live shop RNG is untouched by search!
    g1 = seaglass.TavernGame(seed=333)
    g1.start_turn()
    g1.end_turn()
    g1.start_turn()
    g2 = g1.copy()
    idx_tg = bot.act(g1)
    assert g1.is_legal(action_from_index(idx_tg))

    # Stepping `refresh` on g1 and g2 must produce the exact same shop if g1's RNG was not mutated!
    g1.step(seaglass.TavernAction.refresh())
    g2.step(seaglass.TavernAction.refresh())
    assert [u.card_id for u in g1.observe().shop] == [u.card_id for u in g2.observe().shop]


def test_apex_conditional_engine_gating_and_hand_dedup() -> None:
    kb = get_apex_knowledge("full")
    ev = ApexEvaluator("full", use_neural_value=False)

    # Young Murk-Eye without Battlecry allies on Turn 10 -> 0.0 engine bonus; with Battlecry -> > 15.0!
    murk = seaglass.parse_unit("7/4 card:young_murk_eye")
    vanilla = seaglass.parse_unit("10/10")
    bc_ally = seaglass.parse_unit("4/4 card:dune_dweller")

    b_no_bc = [murk, vanilla]
    assert (
        ev.effective_engine_bonus(
            kb.young_murk_eye_id, 10, {"murloc": 1}, on_board=True, board_units=b_no_bc
        )
        == 0.0
    )
    b_with_bc = [murk, bc_ally]
    assert (
        ev.effective_engine_bonus(
            kb.young_murk_eye_id, 10, {"murloc": 1, "elemental": 1}, on_board=True, board_units=b_with_bc
        )
        >= 15.0
    )

    # Hand symmetry deduplication: 5 identical spells in hand on a 3-minion board -> collapsed to 1 hand index!
    gem = seaglass.parse_unit("1/1")
    class _StubSpell:
        card_id = 999991  # generated token spell (not in templates_by_id)
        is_golden = False
        is_spell = True
        attack = 0
        health = 0
        tavern_tier = 1
        spell_cost = 0
        counter = 0
        unplayable = False
        dark_gifts = ()
        tribes = ()
        magnetic = False
        venomous = False
        divine_shield = False

    class _StubObs:
        turn = 8
        tavern_tier = 4
        gold = 3
        minion_cost = 3
        upgrade_cost = 7
        refresh_cost = 1
        free_refreshes = 0
        is_frozen = False
        hero_id = 0
        trinkets = ()
        board = [vanilla, bc_ally, gem]
        hand = [_StubSpell() for _ in range(5)]
        shop = []
        health = 30
        armor = 5

    legal_plays = [
        seaglass.TavernAction.play(h_i, b_p)
        for h_i in range(5)
        for b_p in range(3)
    ]
    pruned = prune_apex_actions(_StubObs(), legal_plays, ev, kb)  # type: ignore[arg-type]
    assert len(pruned) == 3
    assert {a.args[0] for a in pruned} == {0}


def test_apex_positioner_eot_adjacency() -> None:
    pos = ApexPositioner("full")
    murk = seaglass.parse_unit("7/4 card:young_murk_eye")
    bc = seaglass.parse_unit("3/3 card:dune_dweller")
    filler1 = seaglass.parse_unit("12/12 taunt")
    filler2 = seaglass.parse_unit("8/8")
    board = [murk, filler1, filler2, bc]

    cands = pos._candidate_permutations(board)
    top_perm = cands[0]
    ordered = [board[i] for i in top_perm]
    m_pos = next(i for i, u in enumerate(ordered) if u.card_id == murk.card_id)
    bc_pos = next(i for i, u in enumerate(ordered) if u.card_id == bc.card_id)
    assert abs(m_pos - bc_pos) == 1


def test_apex_value_net_npz_roundtrip_and_default_checkpoint(tmp_path: Path) -> None:
    pred_temp = ApexValuePredictor("full", weights_path=tmp_path / "nonexistent.npz")
    tag_mat = np.asarray(pred_temp.meta.tag_matrix, dtype=np.float32)

    net = ApexValueNet(
        vocab_size=pred_temp.meta.vocab_size,
        tag_matrix=tag_mat,
        global_dim=pred_temp.featurizer.global_dim,
        d_model=32,
        n_heads=4,
        n_layers=2,
    )
    npz_file = tmp_path / "test_weights.npz"
    net.save_npz(npz_file, use_fp16=True)
    assert npz_file.is_file()

    loaded = ApexValueNet.load_npz(npz_file, tag_matrix=tag_mat)
    assert loaded.d_model == 32
    assert loaded.n_layers == 2

    g = seaglass.TavernGame(seed=1234)
    g.start_turn()
    v, s, p, nu, ma, gl = pred_temp.featurizer.featurize_batch([g.observe()], None, True)
    with torch.inference_mode():
        u = loaded.expected_utility(
            torch.from_numpy(v),
            torch.from_numpy(s),
            torch.from_numpy(p),
            torch.from_numpy(nu),
            torch.from_numpy(ma),
            torch.from_numpy(gl),
        )
    assert u.shape == (1,)
    assert np.isfinite(float(u.item()))

    if DEFAULT_APEX_NPZ_PATH.is_file():
        default_pred = ApexValuePredictor("full")
        assert default_pred.is_ready
        vals = default_pred.predict_batch([g.observe()], ctx=None, end_of_turn=True)
        assert len(vals) == 1 and np.isfinite(vals[0])
