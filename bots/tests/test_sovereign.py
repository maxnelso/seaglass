"""Unit, integration, parity, and strict fair-play tests for SovereignBot (`seaglass_bots` + `seaglass_rl`)."""

from __future__ import annotations

from pathlib import Path

import numpy as np
import seaglass
import torch

from seaglass_bots import (
    SovereignBeliefTracker,
    SovereignBot,
    SovereignEvaluator,
    SovereignNumpyPredictor,
    SovereignPositioner,
    SovereignValueNet,
    get_sovereign_knowledge,
)
from seaglass_bots.sovereign_model import DEFAULT_SOVEREIGN_NPZ_PATH
from seaglass_rl.actions import action_from_index
from seaglass_rl.env import BattlegroundsEnv
from seaglass_rl.play import BOTS, GameSpec, LobbyConfig, Player, PlayOptions, Seat


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


def test_sovereign_exports_and_rl_registration() -> None:
    assert "sovereign" in BOTS
    bot = SovereignBot(beam_width=6, max_epoch_depth=4, top_k_combat_evals=2, refresh_samples=1)
    assert isinstance(bot.evaluator, SovereignEvaluator)
    assert isinstance(bot.belief, SovereignBeliefTracker)
    assert isinstance(bot.positioner, SovereignPositioner)


def test_sovereign_8p_ffa_strict_fair_play_proxy() -> None:
    lobby = seaglass.LobbyGame(players=8, duos=False, seed=8181)
    proxy = _StrictFairPlayLobbyProxy(lobby)
    bot = SovereignBot(
        beam_width=6, max_epoch_depth=4, top_k_combat_evals=2, refresh_samples=1, seed=13
    )

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


def test_sovereign_rl_play_8p_ffa_and_rng_isolation() -> None:
    cfg = LobbyConfig(duos=False, players=8, max_turns=5)
    spec = GameSpec(seed=9191, controllers=["sovereign"] + ["heuristic"] * 7)
    player = Player(0)
    try:
        results = player.play(cfg, [spec], {}, PlayOptions(), record=(), seed=9191)
    finally:
        player.close()
    assert len(results) == 1
    assert len(results[0].placements) == 8
    assert set(results[0].placements) == set(range(1, 9))

    bot = SovereignBot(beam_width=6, max_epoch_depth=4, top_k_combat_evals=2, refresh_samples=1, seed=29)
    lobby = seaglass.LobbyGame(players=8, duos=False, seed=121)
    lobby.begin_phase()
    seat = Seat(lobby, 0)
    idx_seat = bot.act(seat)
    assert seat.is_legal(action_from_index(idx_seat))

    env = BattlegroundsEnv(opponent="heuristic", max_turns=4)
    env.reset(seed=232)
    idx_mp = bot.act(env.player)
    assert env.player.is_legal(action_from_index(idx_mp))
    env.step(idx_mp)

    g1 = seaglass.TavernGame(seed=343)
    g1.start_turn()
    g1.end_turn()
    g1.start_turn()
    g2 = g1.copy()
    idx_tg = bot.act(g1)
    assert g1.is_legal(action_from_index(idx_tg))
    g1.step(seaglass.TavernAction.refresh())
    g2.step(seaglass.TavernAction.refresh())
    assert [u.card_id for u in g1.observe().shop] == [u.card_id for u in g2.observe().shop]


def test_sovereign_fast_reposition_and_5_tribe_conditioning() -> None:
    pos = SovereignPositioner("full")
    murk = seaglass.parse_unit("7/4 card:young_murk_eye")
    bc1 = seaglass.parse_unit("3/3 card:dune_dweller")
    bc2 = seaglass.parse_unit("4/4 card:dune_dweller")
    taunt_f = seaglass.parse_unit("15/15 taunt")
    filler1 = seaglass.parse_unit("8/8")
    filler2 = seaglass.parse_unit("9/9")
    titus = seaglass.parse_unit("1/7 card:titus_rivendare")

    # Deliberately awful starting layout: Murk-Eye at far right next to Titus & Taunt!
    board = [bc1, filler1, filler2, bc2, taunt_f, titus, murk]
    perm = pos.fast_best_perm(board)
    ordered = [board[i] for i in perm]
    m_pos = next(i for i, u in enumerate(ordered) if u.card_id == murk.card_id)
    neighbors = []
    if m_pos > 0:
        neighbors.append(ordered[m_pos - 1].card_id)
    if m_pos + 1 < len(ordered):
        neighbors.append(ordered[m_pos + 1].card_id)
    assert bc1.card_id in neighbors

    # Verify 5-tribe conditioning scales Titus down when zero Deathrattle tribes exist vs Beast+Undead+Mech
    sov_kb = get_sovereign_knowledge("full")
    m_none = sov_kb.neutral_engine_tribe_multiplier(titus.card_id, ("elemental", "pirate", "quilboar", "dragon", "murloc"))
    m_rich = sov_kb.neutral_engine_tribe_multiplier(titus.card_id, ("beast", "undead", "mech", "dragon", "pirate"))
    assert m_none < 0.50
    assert m_rich > 1.10

    # Verify SovereignBeliefTracker protects Venomous/Leeroy from eviction on evolved seen boards!
    bt = SovereignBeliefTracker("full", seed=5)
    leeroy = seaglass.parse_unit("6/2 card:leeroy_the_reckless")
    ven = seaglass.parse_unit("2/2 venomous")
    bigs = [seaglass.parse_unit(f"{30+i}/{30+i}") for i in range(5)]
    evolved = bt._evolve_seen_board(
        seen_board=bigs + [leeroy, ven],
        dt=2,
        turn=11,
        tier=5,
        win_streak=1,
        is_ghost=False,
        lobby_tribes=("beast", "dragon", "mech", "pirate", "undead"),
    )
    assert any(u.card_id == leeroy.card_id for u in evolved)
    assert any(u.venomous for u in evolved)


def test_sovereign_numpy_predictor_exact_pytorch_parity(tmp_path: Path) -> None:
    pred_temp = SovereignNumpyPredictor("full", weights_path=tmp_path / "nonexistent.npz")
    tag_mat = np.asarray(pred_temp.meta.tag_matrix, dtype=np.float32)

    torch.manual_seed(99)
    net = SovereignValueNet(
        vocab_size=pred_temp.meta.vocab_size,
        tag_matrix=tag_mat,
        global_dim=pred_temp.featurizer.global_dim,
        d_model=96,
        n_heads=4,
        n_layers=3,
    )
    npz_file = tmp_path / "test_sov_weights.npz"
    net.save_npz(npz_file, use_fp16=True)
    assert npz_file.is_file()

    loaded_pt = SovereignValueNet.load_npz(npz_file, tag_matrix=tag_mat)
    loaded_np = SovereignNumpyPredictor("full", weights_path=npz_file)
    assert loaded_np.is_ready

    lobby = seaglass.LobbyGame(players=8, duos=False, seed=4455)
    lobby.begin_phase()
    for p in list(lobby.active_players()):
        lobby.step(p, lobby.legal_actions(p)[0])
    lobby.end_phase()
    lobby.begin_phase()

    obs_batch = [lobby.observe(p) for p in range(8)]
    ctx0 = lobby.context(0)
    v, s, ps, nu, ma, gl = loaded_np.featurizer.featurize_batch(obs_batch, ctx0, False)

    with torch.inference_mode():
        pt_vals = loaded_pt.expected_utility(
            torch.from_numpy(v),
            torch.from_numpy(s),
            torch.from_numpy(ps),
            torch.from_numpy(nu),
            torch.from_numpy(ma),
            torch.from_numpy(gl),
        ).numpy()

    np_vals = np.asarray(loaded_np.predict_batch(obs_batch, ctx=ctx0, end_of_turn=False), dtype=np.float32)
    max_diff = float(np.max(np.abs(pt_vals - np_vals)))
    assert max_diff < 5e-4, f"Expected <5e-4 parity between NumPy BLAS and PyTorch, got {max_diff}"

    if DEFAULT_SOVEREIGN_NPZ_PATH.is_file():
        default_pred = SovereignNumpyPredictor("full")
        assert default_pred.is_ready
        d_vals = default_pred.predict_batch(obs_batch[:2], ctx=ctx0, end_of_turn=True)
        assert len(d_vals) == 2 and all(np.isfinite(x) for x in d_vals)


def test_sovereign_v3_survival_and_cannibalization_guards() -> None:
    from types import SimpleNamespace
    from seaglass_bots.sovereign_search import _cycle_stat_cap

    # 1. Overkill slope at lethal HP strictly prefers dying at -1 over -15
    u_near = SovereignEvaluator.cvar_survival_utility(-1.0, alive_units=6, lobby_hps=[25] * 6, damage_cap=15)
    u_blowout = SovereignEvaluator.cvar_survival_utility(-15.0, alive_units=6, lobby_hps=[25] * 6, damage_cap=15)
    assert u_near > u_blowout + 20.0

    # 2. Midgame cycle_stat_cap protects 15-30 stat units on Turns 6-8 while opening up on Turn 10+
    assert _cycle_stat_cap(5) == 12
    assert _cycle_stat_cap(6) == 14
    assert _cycle_stat_cap(8) == 20
    assert _cycle_stat_cap(10) >= 34

    # 3. Lone Soul Rewinder on a Demon board is protected from midgame cycle-selling
    ev = SovereignEvaluator("full")
    rew = seaglass.parse_unit("3/4 card:soul_rewinder")
    dem = seaglass.parse_unit("6/6 card:wrath_weaver")
    imp = seaglass.parse_unit("4/4 card:felboar")
    board = [rew, dem, imp]
    eng_lone = ev.effective_engine_bonus(
        rew.card_id, turn=8, tribe_counts={"demon": 2}, on_board=True, board_units=board
    )
    assert eng_lone >= 10.5

    # 4. _select_stratified_roll_prefixes rejects board-cannibalizing [Upgrade, Sell, Sell] paths
    def _mock_obs(tier: int, n_board: int, hp: int, gold: int):
        units = [SimpleNamespace(card_id=101 + i, attack=8, health=8, is_golden=False, divine_shield=False, reborn=False, venomous=False, windfury=False, taunt=False) for i in range(n_board)]
        return SimpleNamespace(
            tavern_tier=tier,
            turn=8,
            board=units,
            hand=[],
            shop=(),
            gold=gold,
            refresh_cost=1,
            free_refreshes=0,
            upgrade_cost=8,
            is_frozen=False,
            hero_power_uses_left=0,
            player_counters=(),
            effects=(),
            trinket_counters=(),
            health=hp,
            armor=0,
            team_health=hp,
            team_armor=0,
            hero_power_used=False,
            trinkets=(),
        )

    root_obs = _mock_obs(tier=4, n_board=7, hp=22, gold=10)
    cannibal_obs = _mock_obs(tier=5, n_board=5, hp=22, gold=3)
    clean_obs = _mock_obs(tier=5, n_board=7, hp=24, gold=2)
    selected = SovereignBot._select_stratified_roll_prefixes(
        roll_prefixes=[
            (95.0, None, cannibal_obs, []),  # type: ignore[arg-type]
            (96.0, None, clean_obs, []),  # type: ignore[arg-type]
        ],
        root_fast=100.0,
        root_obs=root_obs,  # type: ignore[arg-type]
    )
    assert len(selected) == 1
    assert len(selected[0][2].board) == 7

