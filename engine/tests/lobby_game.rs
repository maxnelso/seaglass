//! Full lobbies (`docs/lobby_game.md`): Solo and Duos games played to the end by random bots,
//! every unit placed once, and their replays re-simulating faithfully.

use seaglass::replay::FrameKind;
use seaglass::{LobbyConfig, LobbyGame, Phase, Rng, TavernAction, Timeline};

/// A bot that takes random legal actions, ending its turn with probability 1/4 each step (or
/// after 12 actions). It draws from its own RNG, never the engine's.
fn play(game: &mut LobbyGame, bot: &mut Rng) {
    while !game.is_over() {
        game.begin_phase().unwrap();
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
        game.end_phase().unwrap();
    }
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
