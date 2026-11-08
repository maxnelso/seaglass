//! Full lobbies (`docs/lobby_game.md`): Solo and Duos games played to the end by random bots,
//! every unit placed once, and their replays re-simulating faithfully.

use seaglass::replay::FrameKind;
use seaglass::{LobbyConfig, LobbyGame, Phase, Rng, TavernAction, Timeline};

/// A bot that takes random legal actions, ending its turn with probability 1/4 each step (or
/// after 12 actions). It draws from its own RNG, never the engine's. Checks that no unit takes
/// more combat damage than the turn's damage cap.
fn play(game: &mut LobbyGame, bot: &mut Rng) {
    while !game.is_over() {
        game.begin_phase().unwrap();
        let cap = game.damage_cap();
        for _ in 0..12 {
            let active = game.active_players();
            if active.is_empty() {
                break;
            }
            for p in active {
                let actions = game.valid_actions(p);
                if actions.is_empty() {
                    continue;
                }
                let end = TavernAction::EndTurn;
                let action = if actions.contains(&end) && bot.below(4) == 0 {
                    end
                } else {
                    actions[bot.below(actions.len())]
                };
                game.step(p, action, None).unwrap();
            }
        }
        for p in game.active_players() {
            game.force_end(p).unwrap();
        }
        let before: Vec<i32> = (0..game.num_units())
            .map(|u| {
                let (health, armor) = game.unit_health(u);
                health + armor
            })
            .collect();
        game.end_phase().unwrap();
        if let Some(cap) = cap {
            for (u, &b) in before.iter().enumerate() {
                let (health, armor) = game.unit_health(u);
                assert!(
                    b - (health + armor) <= cap as i32,
                    "turn {}: unit {u} took {} with a cap of {cap}",
                    game.turn(),
                    b - (health + armor)
                );
            }
        }
    }
}

#[test]
fn a_lobby_has_five_minion_types_and_only_their_minions_appear() {
    use seaglass::game::in_lobby;
    use seaglass::model::{Tribe, LOBBY_TRIBES};

    let mut seen_sets = std::collections::BTreeSet::new();
    for seed in 0..4u64 {
        let mut game = LobbyGame::new(LobbyConfig {
            seed,
            heroes: false,
            ..Default::default()
        })
        .unwrap();
        let tribes = game.tribes();
        assert_eq!(tribes.len(), 5);
        assert!(tribes.iter().all(|t| LOBBY_TRIBES.contains(t)));
        seen_sets.insert(format!("{tribes:?}"));
        // Every shop of a whole game offers only lobby minions.
        let mut bot = Rng::new(seed);
        while !game.is_over() {
            game.begin_phase().unwrap();
            for p in 0..game.num_players() {
                for unit in &game.player(p).shop {
                    if !unit.is_spell {
                        assert!(
                            in_lobby(unit.tribe, &tribes),
                            "{} ({:?}) in {tribes:?}",
                            unit.name,
                            unit.tribe
                        );
                    }
                }
            }
            for _ in 0..6 {
                for p in game.active_players() {
                    let actions = game.valid_actions(p);
                    if !actions.is_empty() {
                        game.step(p, actions[bot.below(actions.len())], None)
                            .unwrap();
                    }
                }
            }
            game.end_phase().unwrap();
        }
        assert_ne!(tribes[0], Tribe::None);
    }
    assert!(seen_sets.len() > 1, "every lobby drew the same types");
    // 0 (or 10) types: every type.
    let every = LobbyGame::new(LobbyConfig {
        tribes: 0,
        ..Default::default()
    })
    .unwrap();
    assert!(every.tribes().is_empty());
}

#[test]
fn damage_caps_follow_the_game_rules() {
    let solo = |turn: u32| {
        let mut game = LobbyGame::new(LobbyConfig {
            heroes: false,
            seed: 1,
            ..Default::default()
        })
        .unwrap();
        while game.turn() < turn {
            game.begin_phase().unwrap();
            if game.turn() < turn {
                game.end_phase().unwrap();
            }
        }
        game.damage_cap()
    };
    assert_eq!(solo(1), Some(5));
    assert_eq!(solo(3), Some(5));
    assert_eq!(solo(4), Some(10));
    assert_eq!(solo(8), Some(15));
    let mut duos = LobbyGame::new(LobbyConfig {
        duos: true,
        heroes: false,
        ..Default::default()
    })
    .unwrap();
    duos.begin_phase().unwrap();
    assert_eq!(duos.damage_cap(), Some(15));
    let uncapped = LobbyGame::new(LobbyConfig {
        damage_caps: false,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(uncapped.damage_cap(), None);
}

#[test]
fn solo_and_duos_lobbies_play_to_the_end_and_replay_faithfully() {
    for duos in [false, true] {
        for seed in 0..3u64 {
            let mut game = LobbyGame::new(LobbyConfig {
                duos,
                seed,
                ..Default::default()
            })
            .unwrap();
            play(&mut game, &mut Rng::new(seed ^ 0xB07));
            assert_eq!(game.phase(), Phase::Over);
            let mut places: Vec<u32> = game.placements().iter().map(|p| p.unwrap()).collect();
            places.sort();
            assert_eq!(
                places,
                (1..=game.num_units() as u32).collect::<Vec<_>>(),
                "duos {duos} seed {seed}"
            );

            let timeline = Timeline::new(&game.game().replay()).unwrap();
            assert!(
                timeline.is_faithful(),
                "duos {duos} seed {seed}: {:?} {:?}",
                timeline.divergences.first(),
                timeline.error
            );
            let end = &timeline.frames.last().unwrap().kind;
            let FrameKind::End { placements, .. } = end else {
                panic!("the replay ends with {end:?}");
            };
            let recorded: Vec<u32> = game.placements().iter().map(|p| p.unwrap()).collect();
            assert_eq!(placements, &recorded);
            let combats = timeline
                .frames
                .iter()
                .filter(|f| {
                    matches!(
                        f.kind,
                        FrameKind::Combat { .. } | FrameKind::DuosCombat { .. }
                    )
                })
                .count();
            assert!(combats > 0);
            if duos {
                assert_eq!(timeline.frames[0].teams.len(), 4);
            }
        }
    }
}

#[test]
fn ghosts_fight_once_someone_is_out() {
    // A 3-unit lobby always has an odd one out, who fights a ghost once someone is knocked out.
    let mut seen_ghost = false;
    for seed in 0..6u64 {
        let mut game = LobbyGame::new(LobbyConfig {
            players: 3,
            heroes: false,
            seed,
            ..Default::default()
        })
        .unwrap();
        play(&mut game, &mut Rng::new(seed));
        let timeline = Timeline::new(&game.game().replay()).unwrap();
        assert!(timeline.is_faithful(), "seed {seed}");
        seen_ghost |= timeline
            .frames
            .iter()
            .any(|f| matches!(f.kind, FrameKind::GhostCombat { .. }));
    }
    // With 3 units, a ghost appears only after a knockout while two are left... which ends the
    // game; so instead check 5-player lobbies too.
    for seed in 0..6u64 {
        let mut game = LobbyGame::new(LobbyConfig {
            players: 5,
            heroes: false,
            seed,
            ..Default::default()
        })
        .unwrap();
        play(&mut game, &mut Rng::new(seed));
        let timeline = Timeline::new(&game.game().replay()).unwrap();
        assert!(timeline.is_faithful(), "seed {seed}");
        seen_ghost |= timeline
            .frames
            .iter()
            .any(|f| matches!(f.kind, FrameKind::GhostCombat { .. }));
    }
    assert!(seen_ghost, "no lobby fought a ghost");
}
