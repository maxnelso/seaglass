"""Unit & integration tests for ArchitectBot (CompRegistry, CompPlanner, ArchitectPositioner, LobbyGame)."""

from __future__ import annotations

import seaglass

from seaglass_bots.architect import (
    ALL_COMPS,
    ArchitectBot,
    ArchitectPositioner,
    CompPlanner,
    get_comp_registry,
)


def test_all_comp_cards_resolve_in_catalog() -> None:
    reg = get_comp_registry("full")
    assert len(ALL_COMPS) >= 24
    missing = []
    for comp in ALL_COMPS:
        for bucket in (comp.lynchpins, comp.scalers, comp.multipliers, comp.support):
            for name in bucket:
                if name not in reg.name_to_id:
                    missing.append((comp.comp_id, name))
    assert missing == [], f"Unresolved card names: {missing}"


def test_reborn_undead_butcher_puzzle_completion_and_positioning() -> None:
    reg = get_comp_registry("full")
    planner = CompPlanner("full")
    pos = ArchitectPositioner("full")

    # Build a synthetic TavernState with Snazzy Phantom + Barrier Banshee + Risen Rider
    lobby = seaglass.LobbyGame(players=8, duos=False, seed=42)
    lobby.begin_phase()
    for p in lobby.active_players():
        lobby.step(p, lobby.legal_actions(p)[0])
    lobby.end_phase()
    lobby.begin_phase()

    # Verify comp registry links Snazzy Phantom + Drustfallen Butcher + Butchering
    reborn_comp = reg.by_id["undead_reborn_butcher"]
    assert "Snazzy Phantom" in reborn_comp.lynchpins
    assert "Drustfallen Butcher" in reborn_comp.lynchpins
    assert "Barrier Banshee" in reborn_comp.lynchpins
    assert "Butchering" in reborn_comp.spells

    # Check Poet interior positioning invariant directly
    poet = seaglass.parse_unit("4/6 card:persistent_poet")
    vindicator = seaglass.parse_unit("120/100 card:crimson_vindicator")
    broodmother = seaglass.parse_unit("90/80 card:heroic_broodmother")
    spore = seaglass.parse_unit("1/1 card:deadly_spore")
    board = [poet, vindicator, broodmother, spore]

    obs = lobby.observe(0)
    plan = planner.analyze(obs, lobby.context(0))
    # Verify _score_order strictly prefers Poet between the two Dragons over Poet at slot 0
    poet_edge = pos._score_order(board, [0, 1, 2, 3], obs, plan)
    poet_middle = pos._score_order(board, [1, 0, 2, 3], obs, plan)
    assert poet_middle > poet_edge, f"Expected Poet between 2 Dragons ({poet_middle}) > edge ({poet_edge})"


def test_architect_bot_completes_full_solo_game() -> None:
    lobby = seaglass.LobbyGame(players=8, duos=False, seed=1234)
    bots = {0: ArchitectBot(seed=1234, beam_width=10, max_epoch_depth=6, top_k_combat_evals=4, refresh_samples=3, num_particles=2)}
    from seaglass_bots.baseline import SemiRandomBot

    for p in range(1, 8):
        bots[p] = SemiRandomBot(seed=1234 + p)

    while not lobby.done:
        lobby.begin_phase()
        for p in list(lobby.active_players()):
            steps = 0
            while not lobby.is_done(p) and steps < 80:
                act = bots[p].act_in_lobby(lobby, p)
                assert lobby.is_legal(p, act), f"Illegal action {act} on turn {lobby.turn}"
                note = getattr(bots[p], "last_note", None)
                lobby.step(p, act, note=note)
                steps += 1
            if not lobby.is_done(p):
                lobby.force_end(p)
        lobby.end_phase()

    assert lobby.placements[0] is not None
    assert 1 <= lobby.placements[0] <= 8
