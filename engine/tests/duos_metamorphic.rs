//! Metamorphic checks of Duos combat against Solo (`docs/duos.md`), over random boards of every
//! catalog minion: a Duos combat in which one warband of each team is empty must resolve exactly
//! like the Solo battle between the other two. An empty warband with no hand or player effects
//! steps in with nothing (§3.5), so it adds `WarbandSteppedIn` events and changes nothing else.
//!
//! `SEAGLASS_METAMORPHIC_CASES` sets the number of random cases (default 600).

use seaglass::{
    full_catalog, resolve_battle, resolve_duos_battle, BattleResult, CardTemplate, DuosResult,
    DuosTeam, Event, GameState, PlayerAuras, Rng, Unit, Warband, WarbandResult,
};

fn cases() -> usize {
    std::env::var("SEAGLASS_METAMORPHIC_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600)
}

/// Up to `max` random catalog minions, a fifth of them Golden.
fn random_units(rng: &mut Rng, catalog: &[CardTemplate], max: usize) -> Vec<Unit> {
    (0..rng.below(max + 1))
        .map(|_| {
            let mut unit = catalog[rng.below(catalog.len())].instantiate();
            if rng.below(5) == 0 {
                unit.make_golden();
            }
            unit
        })
        .collect()
}

fn describe(units: &[Unit]) -> String {
    let names: Vec<String> = units
        .iter()
        .map(|u| format!("{} {}/{}", u.name, u.attack, u.health))
        .collect();
    format!("[{}]", names.join(", "))
}

/// One random Solo battle's inputs.
struct Case {
    board_a: Vec<Unit>,
    board_b: Vec<Unit>,
    state: GameState,
    seed: u64,
}

impl Case {
    fn random(rng: &mut Rng, catalog: &[CardTemplate]) -> Self {
        Self {
            board_a: random_units(rng, catalog, 7),
            board_b: random_units(rng, catalog, 7),
            state: GameState {
                hero_tier_a: 1 + rng.below(6) as u32,
                hero_tier_b: 1 + rng.below(6) as u32,
                auras_a: PlayerAuras::default(),
                auras_b: PlayerAuras::default(),
                hand_a: random_units(rng, catalog, 3),
                hand_b: random_units(rng, catalog, 3),
            },
            seed: rng.next_u64(),
        }
    }

    fn describe(&self) -> String {
        format!(
            "seed {}: A {} (hand {}, Tier {}) v B {} (hand {}, Tier {})",
            self.seed,
            describe(&self.board_a),
            describe(&self.state.hand_a),
            self.state.hero_tier_a,
            describe(&self.board_b),
            describe(&self.state.hand_b),
            self.state.hero_tier_b,
        )
    }

    /// A Duos team: this side's warband as player `fighter`, an empty warband as the other
    /// player, and `first` fighting first.
    fn team(&self, side_a: bool, fighter: usize, first: usize) -> DuosTeam {
        let (board, hand, hero_tier) = if side_a {
            (&self.board_a, &self.state.hand_a, self.state.hero_tier_a)
        } else {
            (&self.board_b, &self.state.hand_b, self.state.hero_tier_b)
        };
        let mut warbands: [Warband; 2] = Default::default();
        warbands[fighter] = Warband {
            board: board.clone(),
            hand: hand.clone(),
            auras: PlayerAuras::default(),
            hero_tier,
        };
        DuosTeam { warbands, first }
    }
}

/// Assert that `duos` resolved exactly like `solo`, with team A's warband as player
/// `fighters[0]` and team B's as player `fighters[1]`.
fn assert_same(case: &Case, solo: &BattleResult, duos: &DuosResult, fighters: [usize; 2]) {
    let context = case.describe();
    assert_eq!(duos.outcome, solo.outcome, "outcome, {context}");
    assert_eq!(duos.hero_damage, solo.hero_damage, "damage, {context}");
    let events: Vec<Event> = duos
        .events
        .iter()
        .filter(|e| !matches!(e, Event::WarbandSteppedIn { .. }))
        .cloned()
        .collect();
    if events != solo.events {
        let at = events
            .iter()
            .zip(&solo.events)
            .position(|(d, s)| d != s)
            .unwrap_or(events.len().min(solo.events.len()));
        panic!(
            "events differ at {at}: Duos {:?}, Solo {:?}; {context}",
            events.get(at),
            solo.events.get(at)
        );
    }
    let check = |result: &WarbandResult,
                 survivors: &[Unit],
                 hand: &[Unit],
                 auras: &PlayerAuras,
                 dead: &[Unit]| {
        assert_eq!(result.survivors, survivors, "survivors, {context}");
        assert_eq!(result.hand, hand, "hand, {context}");
        assert_eq!(&result.auras, auras, "auras, {context}");
        assert_eq!(result.dead_units, dead, "dead units, {context}");
    };
    check(
        &duos.team_a[fighters[0]],
        &solo.survivors_a,
        &solo.hand_a,
        &solo.auras_a,
        &solo.dead_units_a,
    );
    check(
        &duos.team_b[fighters[1]],
        &solo.survivors_b,
        &solo.hand_b,
        &solo.auras_b,
        &solo.dead_units_b,
    );
    for empty in [&duos.team_a[1 - fighters[0]], &duos.team_b[1 - fighters[1]]] {
        assert!(empty.survivors.is_empty(), "empty warband, {context}");
        assert!(empty.dead_units.is_empty(), "empty warband, {context}");
    }
}

/// The empty warbands fight second: each steps in, with nothing, once its teammate runs out.
#[test]
fn empty_second_warbands_resolve_like_the_solo_battle() {
    let catalog = full_catalog();
    let mut rng = Rng::new(0xD005);
    for _ in 0..cases() {
        let case = Case::random(&mut rng, &catalog);
        let fighters = [rng.below(2), rng.below(2)];
        let solo = resolve_battle(&case.board_a, &case.board_b, &case.state, case.seed);
        let team_a = case.team(true, fighters[0], fighters[0]);
        let team_b = case.team(false, fighters[1], fighters[1]);
        let duos = resolve_duos_battle(&team_a, &team_b, case.seed);
        assert_same(&case, &solo, &duos, fighters);
        assert_eq!(
            resolve_duos_battle(&team_a, &team_b, case.seed),
            duos,
            "determinism, {}",
            case.describe()
        );
    }
}

/// The empty warbands fight first: they open with nothing, so both teammates step in together
/// and their Start of Combat resolves as at the start of the Solo battle.
#[test]
fn empty_first_warbands_resolve_like_the_solo_battle() {
    let catalog = full_catalog();
    let mut rng = Rng::new(0xF175);
    for _ in 0..cases() {
        let case = Case::random(&mut rng, &catalog);
        let fighters = [rng.below(2), rng.below(2)];
        let solo = resolve_battle(&case.board_a, &case.board_b, &case.state, case.seed);
        let team_a = case.team(true, fighters[0], 1 - fighters[0]);
        let team_b = case.team(false, fighters[1], 1 - fighters[1]);
        let duos = resolve_duos_battle(&team_a, &team_b, case.seed);
        assert_same(&case, &solo, &duos, fighters);
    }
}
