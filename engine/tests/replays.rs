//! Replay round trips (`docs/replays.md`): a recorded match re-simulates into exactly the states
//! it went through, and tampered or broken replays are flagged at the right call.

use seaglass::lobby::Lobby;
use seaglass::replay::{self, pb, FrameKind, Timeline};
use seaglass::{
    heroes, CardPool, DeityKind, Match, MatchSetup, PlayerSetup, PoolMode, Rng, TavernAction,
    TavernState,
};

/// Everything a frame captures, taken from a live match.
#[derive(Debug, PartialEq)]
struct Snapshot {
    players: Vec<TavernState>,
    rng_states: Vec<u64>,
    pool_remaining: Vec<Vec<u32>>,
}

fn snapshot(game: &Match) -> Snapshot {
    Snapshot {
        players: game.players().to_vec(),
        rng_states: game.rngs().iter().map(Rng::state).collect(),
        pool_remaining: game.pools().iter().map(CardPool::remaining).collect(),
    }
}

fn setup(seed: u64, pool: PoolMode) -> MatchSetup {
    MatchSetup {
        catalog: "full".to_string(),
        pool,
        shared_seed: seed,
        players: vec![
            PlayerSetup {
                name: "p1".to_string(),
                seed,
                deity: DeityKind::CThun,
                ..Default::default()
            },
            PlayerSetup {
                name: "p2".to_string(),
                seed: seed + 1,
                deity: DeityKind::YShaarj,
                shop_spells: !seed.is_multiple_of(3),
                // A different hero (and Hero Power) for each seed.
                hero: Some(hero(seed)),
                ..Default::default()
            },
        ],
        meta: [("source".to_string(), "tests/replays.rs".to_string())].into(),
    }
}

/// The `seed`-th offerable Solo hero.
fn hero(seed: u64) -> seaglass::HeroId {
    let heroes = heroes::offerable(false, true);
    heroes[seed as usize % heroes.len()]
}

/// A random but legal action, drawn from the bot's own RNG: after 25 steps, End Turn as soon as
/// it is legal (a pending prompt — a Discover, a trinket offer, ... — first takes an option).
fn random_action(state: &TavernState, steps: usize, bot: &mut Rng) -> TavernAction {
    if steps >= 25 && state.is_legal(&TavernAction::EndTurn) {
        return TavernAction::EndTurn;
    }
    let actions = state.valid_actions();
    actions[bot.below(actions.len())]
}

/// Play a random match, returning it and a snapshot before the first call and after every call.
fn play(seed: u64, pool: PoolMode) -> (Match, Vec<Snapshot>) {
    let mut game = Match::new(&setup(seed, pool)).unwrap();
    let mut bot = Rng::new(seed ^ 0x05EE_DB07);
    let mut snapshots = vec![snapshot(&game)];
    for turn in 1..=14u64 {
        game.set_pairs(&[(0, 1)]);
        snapshots.push(snapshot(&game));
        for p in 0..2 {
            game.start_turn(p);
            snapshots.push(snapshot(&game));
        }
        game.deliver_gifts();
        snapshots.push(snapshot(&game));
        for p in 0..2 {
            for steps in 0.. {
                let action = random_action(game.player(p), steps, &mut bot);
                let note = (steps % 2 == 0).then(|| pb::Note {
                    value: Some(steps as f32 / 10.0),
                    logp: Some(-(steps as f32)),
                    ..Default::default()
                });
                let ended = game.step_with_note(p, action, note).unwrap();
                snapshots.push(snapshot(&game));
                if ended {
                    break;
                }
            }
        }
        // Both seed sources: drawn from player 0's RNG, or given by the caller.
        if turn % 4 == 3 {
            game.combat_with_seed(0, 1, seed * 1_000 + turn);
        } else {
            game.combat(0, 1);
        }
        snapshots.push(snapshot(&game));
        let alive = game.record_deaths();
        snapshots.push(snapshot(&game));
        if alive.contains(&false) {
            break;
        }
    }
    game.finish();
    snapshots.push(snapshot(&game));
    (game, snapshots)
}

fn assert_frames_match(timeline: &Timeline, snapshots: &[Snapshot], label: &str) {
    assert_eq!(
        timeline.frames.len(),
        snapshots.len(),
        "{label}: frame count"
    );
    for (i, (frame, live)) in timeline.frames.iter().zip(snapshots).enumerate() {
        let rebuilt = Snapshot {
            players: frame.players.clone(),
            rng_states: frame.rng_states.clone(),
            pool_remaining: frame.pool_remaining.clone(),
        };
        assert!(
            rebuilt == *live,
            "{label}: frame {i} differs from the live match"
        );
    }
}

#[test]
fn replays_rebuild_every_state() {
    for pool in [PoolMode::PerPlayer, PoolMode::Shared] {
        for seed in 0..24 {
            let label = format!("{pool:?} seed {seed}");
            let (game, snapshots) = play(seed, pool);
            let recorded = game.replay();
            let decoded = replay::decode(&replay::encode(&recorded)).unwrap();
            assert_eq!(decoded, recorded, "{label}: protobuf round trip");

            let timeline = Timeline::new(&decoded).unwrap();
            assert!(
                timeline.is_faithful(),
                "{label}: {:?} {:?}",
                timeline.divergences,
                timeline.error
            );
            assert_eq!(timeline.engine_mismatch(), None);
            assert_frames_match(&timeline, &snapshots, &label);

            // Frames line up with records, and notes survive.
            for (i, frame) in timeline.frames.iter().enumerate() {
                assert_eq!(frame.record, i.checked_sub(1), "{label}: frame {i}");
            }
            let acts: Vec<&pb::Act> = recorded
                .records
                .iter()
                .filter_map(|r| match &r.kind {
                    Some(pb::record::Kind::Act(act)) => Some(act),
                    _ => None,
                })
                .collect();
            let decisions: Vec<_> = timeline.decisions().collect();
            assert_eq!(decisions.len(), acts.len(), "{label}: decisions");
            for (decision, act) in decisions.iter().zip(&acts) {
                assert_eq!(decision.note, act.note.as_ref());
                assert_eq!(decision.card_id, act.card_id);
                assert_eq!(
                    decision.before,
                    &timeline.frames[decision.frame - 1].players[..]
                );
            }
            assert!(matches!(
                timeline.frames.last().unwrap().kind,
                FrameKind::End { .. }
            ));

            // The replay of a game cut short (a crash, a killed process) rebuilds its prefix.
            let cut = recorded.records.len() / 2;
            let partial = pb::Replay {
                header: recorded.header.clone(),
                records: recorded.records[..cut].to_vec(),
            };
            let timeline = Timeline::new(&partial).unwrap();
            assert!(timeline.is_faithful(), "{label}: partial replay");
            assert_frames_match(&timeline, &snapshots[..=cut], &format!("{label} (partial)"));
        }
    }
}

/// A match drives the engine exactly like calling it directly with the same seeds, so moving a
/// game loop onto `Match` does not change its games.
#[test]
fn match_plays_like_the_bare_engine() {
    for pool in [PoolMode::PerPlayer, PoolMode::Shared] {
        for seed in 100..108 {
            let setup = setup(seed, pool);
            let mut game = Match::new(&setup).unwrap();
            let mut states: Vec<TavernState> = setup
                .players
                .iter()
                .map(|p| {
                    let mut s = TavernState::new().with_shop_spells(p.shop_spells);
                    s.trinkets_enabled = p.trinkets;
                    s.dark_gifts_enabled = p.dark_gifts;
                    s.auras.deity.kind = p.deity;
                    match p.hero {
                        Some(id) => s.set_hero(id).unwrap(),
                        None => s.armor = p.armor,
                    }
                    s
                })
                .collect();
            let mut lobby = Lobby::new(2, false);
            let (mut pools, mut rngs) = match pool {
                PoolMode::PerPlayer => (
                    vec![CardPool::new(seaglass::full_catalog()); 2],
                    vec![Rng::new(seed), Rng::new(seed + 1)],
                ),
                PoolMode::Shared => (
                    vec![CardPool::new(seaglass::full_catalog())],
                    vec![Rng::new(seed)],
                ),
            };
            let slot = |p: usize| if pool == PoolMode::Shared { 0 } else { p };
            let mut bot = Rng::new(seed);
            for _ in 0..10 {
                game.set_pairs(&[(0, 1)]);
                lobby.set_next_pairs(&[(0, 1)]);
                lobby.apply(&mut states.iter_mut().collect::<Vec<_>>());
                for p in 0..2 {
                    game.start_turn(p);
                    states[p].start_turn(&mut pools[slot(p)], &mut rngs[slot(p)]);
                }
                game.deliver_gifts();
                lobby.deliver_gifts(&mut states.iter_mut().collect::<Vec<_>>());
                assert!(
                    game.players() == &states[..],
                    "seed {seed}: after the gifts"
                );
                for p in 0..2 {
                    for steps in 0.. {
                        let action = random_action(&states[p], steps, &mut bot);
                        let ended = game.step(p, action).unwrap();
                        assert_eq!(
                            states[p].step(action, &mut pools[slot(p)], &mut rngs[slot(p)]),
                            Ok(ended)
                        );
                        assert!(game.player(p) == &states[p], "seed {seed}: player {p}");
                        if ended {
                            break;
                        }
                    }
                }
                let combat_seed = rngs[slot(0)].next_u64();
                let boards = [states[0].board.clone(), states[1].board.clone()];
                let (a, b) = states.split_at_mut(1);
                let expected = TavernState::resolve_combat_pair(&mut a[0], &mut b[0], combat_seed);
                assert!(game.combat(0, 1) == expected, "seed {seed}: combat");
                assert!(game.players() == &states[..], "seed {seed}: after combat");
                lobby.record_combat(0, Some(1), &boards[0]);
                lobby.record_combat(1, Some(0), &boards[1]);
                lobby.record_outcome(0, 1, expected.outcome);
                let alive: Vec<bool> = states.iter().map(|s| s.health > 0).collect();
                lobby.record_deaths(&alive);
                assert_eq!(game.record_deaths(), alive);
                assert!(game.lobby() == &lobby, "seed {seed}: lobby");
                if alive.contains(&false) {
                    break;
                }
            }
        }
    }
}

#[test]
fn illegal_actions_are_not_recorded() {
    let mut game = Match::new(&setup(7, PoolMode::PerPlayer)).unwrap();
    game.start_turn(0);
    let records = game.records().len();
    assert!(game.step(0, TavernAction::Buy { shop_index: 99 }).is_err());
    assert_eq!(game.records().len(), records);
}

#[test]
fn setups_round_trip_through_headers() {
    for pool in [PoolMode::PerPlayer, PoolMode::Shared] {
        let setup = setup(5, pool);
        let header = setup.to_header();
        assert_eq!(header.format, replay::FORMAT_VERSION);
        assert_eq!(header.engine, replay::engine_version());
        assert_eq!(MatchSetup::from_header(&header).unwrap(), setup);
    }

    let mut header = setup(5, PoolMode::Shared).to_header();
    header.format = replay::FORMAT_VERSION + 1;
    assert!(MatchSetup::from_header(&header).is_err());
}

/// The index of the first record matching `pick`.
fn find_record(replay: &pb::Replay, pick: impl Fn(&pb::record::Kind) -> bool) -> usize {
    replay
        .records
        .iter()
        .position(|r| r.kind.as_ref().is_some_and(&pick))
        .expect("the replay has a matching record")
}

#[test]
fn a_tampered_buy_is_flagged() {
    let (game, _) = play(3, PoolMode::PerPlayer);
    let mut replay = game.replay();
    let timeline = Timeline::new(&replay).unwrap();

    // Buy a different card than the one recorded: a different shop slot with a different card.
    let (record, slot) = replay
        .records
        .iter()
        .enumerate()
        .find_map(|(i, r)| {
            let Some(pb::record::Kind::Act(act)) = &r.kind else {
                return None;
            };
            let Some(pb::action::Kind::Buy(bought)) = act.action.as_ref()?.kind else {
                return None;
            };
            let before = &timeline.frames[i].players[act.player as usize];
            (0..before.shop.len())
                .find(|&s| {
                    s != bought as usize
                        && before.is_legal(&TavernAction::Buy { shop_index: s })
                        && Some(before.shop[s].card_id) != act.card_id
                })
                .map(|s| (i, s))
        })
        .expect("some buy had an alternative");
    let Some(pb::record::Kind::Act(act)) = &mut replay.records[record].kind else {
        unreachable!()
    };
    act.action = Some(replay::action_to_pb(&TavernAction::Buy {
        shop_index: slot,
    }));

    let tampered = Timeline::new(&replay).unwrap();
    let first = tampered
        .divergences
        .first()
        .expect("the tampering is flagged");
    assert_eq!(first.record, record);
    assert_eq!(first.frame, record + 1);
    assert!(first.message.starts_with("card"), "{}", first.message);
}

#[test]
fn a_tampered_combat_seed_is_flagged() {
    let (game, _) = play(4, PoolMode::Shared);
    let mut replay = game.replay();
    let record = find_record(
        &replay,
        |k| matches!(k, pb::record::Kind::Combat(c) if !c.external_seed),
    );
    let Some(pb::record::Kind::Combat(combat)) = &mut replay.records[record].kind else {
        unreachable!()
    };
    combat.seed ^= 1;

    let tampered = Timeline::new(&replay).unwrap();
    let first = tampered
        .divergences
        .first()
        .expect("the tampering is flagged");
    assert_eq!(first.record, record);
    assert!(
        first.message.starts_with("combat seed"),
        "{}",
        first.message
    );
}

#[test]
fn an_illegal_record_ends_the_timeline() {
    let (game, _) = play(5, PoolMode::PerPlayer);
    let mut replay = game.replay();
    let record = find_record(&replay, |k| matches!(k, pb::record::Kind::Act(_)));
    replay.records.insert(
        record,
        pb::Record {
            kind: Some(pb::record::Kind::Act(pb::Act {
                player: 0,
                action: Some(replay::action_to_pb(&TavernAction::Buy { shop_index: 99 })),
                ..Default::default()
            })),
        },
    );
    let timeline = Timeline::new(&replay).unwrap();
    let error = timeline
        .error
        .as_ref()
        .expect("the illegal call is reported");
    assert_eq!(error.record, record);
    assert!(error.message.contains("illegal"), "{}", error.message);
    assert_eq!(timeline.frames.len(), record + 1);

    // A record for a player that does not exist.
    let mut replay = game.replay();
    replay.records.insert(
        0,
        pb::Record {
            kind: Some(pb::record::Kind::StartTurn(pb::StartTurn {
                player: 7,
                turn: 1,
            })),
        },
    );
    let timeline = Timeline::new(&replay).unwrap();
    assert_eq!(timeline.error.as_ref().map(|e| e.record), Some(0));
    assert_eq!(timeline.frames.len(), 1);
}
