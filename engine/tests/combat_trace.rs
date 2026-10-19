//! Combat traces (`combat::with_combat_trace`): the boards after every strike, for viewers.

use seaglass::{
    full_catalog, resolve_battle, resolve_duos_battle, with_combat_trace, DuosTeam, Event,
    GameState, Rng, Unit, Warband,
};

/// A random board of up to 7 minions from the full catalog.
fn board(rng: &mut Rng) -> Vec<Unit> {
    let minions: Vec<_> = full_catalog()
        .into_iter()
        .filter(|t| t.tavern_tier <= 6)
        .collect();
    (0..1 + rng.below(7))
        .map(|_| minions[rng.below(minions.len())].instantiate())
        .collect()
}

#[test]
fn a_trace_follows_the_combat_without_changing_it() {
    let mut rng = Rng::new(7);
    let state = GameState::default();
    for seed in 0..200 {
        let (a, b) = (board(&mut rng), board(&mut rng));
        let plain = resolve_battle(&a, &b, &state, seed);
        let (traced, snapshots) = with_combat_trace(|| resolve_battle(&a, &b, &state, seed));
        assert_eq!(traced, plain, "seed {seed}: the trace changed the combat");

        // Before Start of Combat: the boards as given, with combat ids.
        assert!(snapshots.len() >= 2, "seed {seed}");
        let names = |units: &[Unit]| units.iter().map(|u| u.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&snapshots[0].board_a), names(&a));
        assert_eq!(names(&snapshots[0].board_b), names(&b));
        // Snapshots move forward through the events, one after every strike.
        for pair in snapshots.windows(2) {
            assert!(pair[0].events <= pair[1].events, "seed {seed}");
        }
        assert!(snapshots.last().unwrap().events <= plain.events.len());
        let strikes = plain
            .events
            .iter()
            .filter(|e| matches!(e, Event::AttackDeclared { .. }))
            .count();
        assert!(
            snapshots.len() <= 2 + strikes,
            "seed {seed}: one snapshot per strike at most"
        );
    }
    // Outside a trace nothing is recorded, and a nested trace does not leak into its parent.
    let (_, outer) = with_combat_trace(|| {
        let (_, inner) = with_combat_trace(|| resolve_battle(&[], &[], &state, 1));
        assert_eq!(inner.len(), 2);
    });
    assert!(outer.is_empty());
}

#[test]
fn a_duos_trace_starts_before_start_of_combat_and_shows_every_warband() {
    let mut rng = Rng::new(11);
    for seed in 0..100 {
        let team = |rng: &mut Rng| DuosTeam {
            warbands: [Warband::new(board(rng)), Warband::new(board(rng))],
            first: rng.below(2),
        };
        let (a, b) = (team(&mut rng), team(&mut rng));
        let plain = resolve_duos_battle(&a, &b, seed);
        let (traced, snapshots) = with_combat_trace(|| resolve_duos_battle(&a, &b, seed));
        assert_eq!(traced, plain, "seed {seed}: the trace changed the combat");
        // The first warbands, as given, before anything happens.
        let names = |units: &[Unit]| units.iter().map(|u| u.name.clone()).collect::<Vec<_>>();
        assert_eq!(
            names(&snapshots[0].board_a),
            names(&a.warbands[a.first].board)
        );
        assert_eq!(
            names(&snapshots[0].board_b),
            names(&b.warbands[b.first].board)
        );
        assert!(snapshots.windows(2).all(|w| w[0].events <= w[1].events));
        // Every warband that stepped in shows up after its entry.
        for (i, e) in plain.events.iter().enumerate() {
            if matches!(e, Event::WarbandSteppedIn { .. }) {
                assert!(snapshots.iter().any(|s| s.events > i), "seed {seed}");
            }
        }
    }
}
