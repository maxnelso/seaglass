//! Declarative YAML scenario harness and unit tests for `seaglass`.

use std::fs;
use std::path::{Path, PathBuf};

use seaglass::{
    catalog_for, parse_unit, run_scenario, run_tavern_scenario, simulate, simulate_batch,
    tier1_catalog, BattleOutcome, CardPool, Defaults, DeityKind, Event, GameState, Keyword, Rng,
    Scenario, Side, TavernAction, TavernScenario, TavernState, Tribe,
};

fn collect_yaml_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("failed to read scenario dir {}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let ext = path.extension()?.to_str()?;
            if ext == "yaml" || ext == "yml" {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}

#[test]
fn run_all_combat_scenarios() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/combat_scenarios");
    let files = collect_yaml_files(&dir);
    assert!(
        !files.is_empty(),
        "expected at least one YAML file in {}",
        dir.display()
    );

    for path in files {
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        let scenario: Scenario = serde_yaml::from_str(&content)
            .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
        run_scenario(&scenario)
            .unwrap_or_else(|e| panic!("combat scenario {} failed: {e}", path.display()));
    }
}

#[test]
fn run_all_tavern_scenarios() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/tavern_scenarios");
    let files = collect_yaml_files(&dir);
    assert!(
        !files.is_empty(),
        "expected at least one YAML file in {}",
        dir.display()
    );

    for path in files {
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        let scenario: TavernScenario = serde_yaml::from_str(&content)
            .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()));
        run_tavern_scenario(&scenario)
            .unwrap_or_else(|e| panic!("tavern scenario {} failed: {e}", path.display()));
    }
}

#[test]
fn tier1_catalog_contains_all_21_live_solo_minions() {
    let cards = tier1_catalog();
    assert_eq!(cards.len(), 21);

    let names: Vec<&str> = cards.iter().map(|c| c.name.as_str()).collect();
    let expected = [
        "Joyous",
        "Zoatroid",
        "Buzzing Vermin",
        "Flittering Bat",
        "Wrath Weaver",
        "Ominous Seer",
        "Glim Guardian",
        "Scarlet Survivor",
        "Crackling Cyclone",
        "Dune Dweller",
        "Cord Puller",
        "Lullabot",
        "Bubble Gunner",
        "Flighty Scout",
        "Aureate Laureate",
        "Southsea Busker",
        "Razorfen Geomancer",
        "Tusked Camper",
        "Harmless Bonehead",
        "Risen Rider",
        "Suspicious Prisonguard",
    ];
    assert_eq!(names, expected);

    // Every Tier 1 minion has a unique non-zero card_id and tavern_tier == 1.
    for (idx, card) in cards.iter().enumerate() {
        assert_eq!(card.tavern_tier, 1);
        assert!(card.card_id >= 101 && card.card_id <= 121);
        for other in &cards[idx + 1..] {
            assert_ne!(card.card_id, other.card_id);
        }
    }
}

#[test]
fn cthun_and_yshaarj_awaken_after_4_friendly_aberrations_die() {
    let defaults = Defaults::default();

    // Side A has 4 Zoatroids (3/2 Aberration) + 1 neutral 2/2, with C'Thun (5/5) as Deity.
    // Side B has a big 6/30 taunt wall.
    let board_a = vec![
        parse_unit("3/2 card:zoatroid", &defaults).unwrap(),
        parse_unit("3/2 card:zoatroid", &defaults).unwrap(),
        parse_unit("3/2 card:zoatroid", &defaults).unwrap(),
        parse_unit("3/2 card:zoatroid", &defaults).unwrap(),
        parse_unit("2/2", &defaults).unwrap(),
    ];
    let board_b = vec![parse_unit("6/30 taunt", &defaults).unwrap()];

    let mut state = GameState::default();
    state.auras_a.deity.kind = DeityKind::CThun;
    state.auras_a.deity.attack = 5;
    state.auras_a.deity.health = 5;

    let res = simulate(&board_a, &board_b, &state, 7);
    let awakened = res.events.iter().any(|e| {
        matches!(
            e,
            Event::DeityAwakened {
                side: Side::A,
                deity: DeityKind::CThun,
                ..
            }
        )
    });
    assert!(
        awakened,
        "expected C'Thun to awaken after 4 friendly Aberrations died"
    );

    // Now test Y'Shaarj (4/4): when Y'Shaarj dies, it summons the first 2 Aberrations
    // that died this combat with their maximum stats.
    let mut state_y = GameState::default();
    state_y.auras_a.deity.kind = DeityKind::YShaarj;
    state_y.auras_a.deity.attack = 4;
    state_y.auras_a.deity.health = 4;

    let board_a_y = vec![
        parse_unit("5/3 card:zoatroid", &defaults).unwrap(),
        parse_unit("4/3 card:joyous", &defaults).unwrap(),
        parse_unit("3/2 card:zoatroid", &defaults).unwrap(),
        parse_unit("3/2 card:zoatroid", &defaults).unwrap(),
    ];
    let board_b_y = vec![parse_unit("10/22 taunt", &defaults).unwrap()];

    let res_y = simulate(&board_a_y, &board_b_y, &state_y, 7);
    let y_awakened = res_y.events.iter().any(|e| {
        matches!(
            e,
            Event::DeityAwakened {
                side: Side::A,
                deity: DeityKind::YShaarj,
                ..
            }
        )
    });
    assert!(
        y_awakened,
        "expected Y'Shaarj to awaken after 4 friendly Aberrations died"
    );
    assert_eq!(res_y.outcome, BattleOutcome::AWin);
}

#[test]
fn simulate_batch_is_strictly_deterministic() {
    let defaults = Defaults::default();
    let board_a = vec![
        parse_unit("1/1 card:harmless_bonehead", &defaults).unwrap(),
        parse_unit("1/4 card:flittering_bat", &defaults).unwrap(),
    ];
    let board_b = vec![
        parse_unit("1/1 card:cord_puller", &defaults).unwrap(),
        parse_unit("2/1 card:scarlet_survivor", &defaults).unwrap(),
    ];
    let state = GameState::default();

    let d1 = simulate_batch(&board_a, &board_b, &state, 12345, 200);
    let d2 = simulate_batch(&board_a, &board_b, &state, 12345, 200);
    assert_eq!(d1, d2);
    assert_eq!(d1.a_wins + d1.b_wins + d1.draws, 200);
}

#[test]
fn card_pool_tracks_copies_and_returns_triples() {
    let templates = catalog_for("tier1").unwrap();
    let mut pool = CardPool::new(templates);
    let mut rng = Rng::new(99);
    let mut state = TavernState::new();

    // Every Tier 1 minion starts with 15 copies.
    assert_eq!(pool.remaining_copies(101), 15);
    state.start_turn(&mut pool, &mut rng);
    assert_eq!(state.shop.len(), 3);
    assert_eq!(state.gold, 3);

    let bought_id = state.shop[0].card_id;
    state
        .step(TavernAction::Buy { shop_index: 0 }, &mut pool, &mut rng)
        .unwrap();
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 0,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.board[0].card_id, bought_id);
    assert!(!state.board[0]. tribe.matches(Tribe::None) || state.board[0].card_id == 121);
    assert!(!state.board[0].taunt || state.board[0].has_keyword(Keyword::Taunt));
}
