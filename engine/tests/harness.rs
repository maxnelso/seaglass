//! Declarative YAML scenario harness and unit tests for `seaglass`.

use std::fs;
use std::path::{Path, PathBuf};

use seaglass::cards::{spells, tier1, tier2, tier3, tokens};
use seaglass::{
    catalog_for, full_catalog, parse_unit, run_scenario, run_tavern_scenario, simulate,
    simulate_batch, tier1_catalog, tier2_catalog, tier3_catalog, tier4_catalog, tier5_catalog,
    tier6_catalog, tier7_catalog, BattleOutcome, CardPool, Defaults, DeityKind, Event, GameState,
    Keyword, Rng, Scenario, Side, TavernAction, TavernScenario, TavernState, Tribe, Unit,
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
fn tier2_catalog_contains_all_34_live_solo_minions_and_15_spells() {
    let cards = tier2_catalog();
    assert_eq!(cards.len(), 34);
    assert_eq!(full_catalog().len(), 252);

    let names: Vec<&str> = cards.iter().map(|c| c.name.as_str()).collect();
    let expected = [
        "Bilgewater Breakout",
        "Blue Volumizer",
        "Brain Rotter",
        "Bronze Warden",
        "Clever Castaway",
        "Crater Miner",
        "Decoy Conjurer",
        "Electric Synthesizer",
        "Eternal Knight",
        "Expert Aviator",
        "Fire Baller",
        "Forest Rover",
        "Green Volumizer",
        "Humming Bird",
        "Intrepid Botanist",
        "Laboratory Assistant",
        "Lurking Lionfish",
        "Mechagnome Interpreter",
        "Mind Muck",
        "Nerubian Deathswarmer",
        "Patient Scout",
        "Prodigious Tusker",
        "Red Volumizer",
        "Roadboar",
        "Scarlet Skull",
        "Sellemental",
        "Snow Baller",
        "Soul Rewinder",
        "Surfing Sylvar",
        "Tad",
        "Tarecgosa",
        "Underrot Spawn",
        "Very Hungry Winterfinner",
        "Wandering Willbreaker",
    ];
    assert_eq!(names, expected);

    for (idx, card) in cards.iter().enumerate() {
        assert_eq!(card.tavern_tier, 2);
        assert!(card.card_id >= 201 && card.card_id <= 234);
        for other in &cards[idx + 1..] {
            assert_ne!(card.card_id, other.card_id);
        }
    }

    assert_eq!(spells::tier1_spells().len(), 8);
    assert_eq!(spells::tier2_spells().len(), 7);
    assert_eq!(spells::spells_up_to_tier(2).len(), 15);
}

#[test]
fn tier3_catalog_contains_all_43_live_solo_minions_and_15_spells() {
    let cards = tier3_catalog();
    assert_eq!(cards.len(), 43);

    for (idx, card) in cards.iter().enumerate() {
        assert_eq!(card.tavern_tier, 3);
        assert!(card.card_id >= 301 && card.card_id <= 343);
        for other in &cards[idx + 1..] {
            assert_ne!(card.card_id, other.card_id);
        }
    }

    assert_eq!(spells::tier3_spells().len(), 15);
    assert_eq!(spells::spells_up_to_tier(3).len(), 30);
}

#[test]
fn cthun_and_yshaarj_awaken_after_4_friendly_aberrations_die() {
    let defaults = Defaults::default();

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
    assert!(!state.board[0].tribe.matches(Tribe::None) || state.board[0].card_id == 121);
    assert!(!state.board[0].taunt || state.board[0].has_keyword(Keyword::Taunt));
}

#[test]
fn tier2_combat_mechanics_work_end_to_end() {
    let defaults = Defaults::default();

    // 1. Electric Synthesizer + Tarecgosa + Prodigious Tusker + Roadboar + Expert Aviator
    let board_a = vec![
        parse_unit("2/4 card:roadboar", &defaults).unwrap(),
        parse_unit("3/5 card:expert_aviator", &defaults).unwrap(),
        parse_unit("3/4 card:electric_synthesizer", &defaults).unwrap(),
        parse_unit("4/4 card:tarecgosa", &defaults).unwrap(),
        parse_unit("2/5 card:prodigious_tusker", &defaults).unwrap(),
    ];
    // Defender has Very Hungry Winterfinner + Underrot Spawn
    let board_b = vec![
        parse_unit("2/6 card:very_hungry_winterfinner", &defaults).unwrap(),
        parse_unit("2/2 card:underrot_spawn", &defaults).unwrap(),
    ];

    let mut state = GameState::default();
    // Put a 6/6 Murloc in Side A's hand for Expert Aviator to summon,
    // and a 2/2 Murloc in Side B's hand for Very Hungry Winterfinner to buff.
    state
        .hand_a
        .push(Unit::new("Hand Murloc", 6, 6).with_tribe(Tribe::Murloc));
    state
        .hand_b
        .push(Unit::new("B Hand Minion", 2, 2).with_tribe(Tribe::Murloc));

    let res = simulate(&board_a, &board_b, &state, 42);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Roadboar generated at least 1 Blood Gem into Side A's hand.
    assert!(res
        .hand_a
        .iter()
        .any(|c| c.card_id == tokens::SPELL_BLOOD_GEM));
    // Very Hungry Winterfinner took damage and buffed Side B's hand minion from 2/2 -> at least 4/3.
    assert!(res.hand_b[0].attack >= 4 && res.hand_b[0].health >= 3);

    // 2. Underrot Spawn summons 0/2 Tentacle with Taunt first, then gives friendly minions +1 Attack
    // so the summoned Tentacle is 1/2 with Taunt.
    let board_u_a = vec![
        parse_unit("2/2 card:underrot_spawn", &defaults).unwrap(),
        parse_unit("1/5", &defaults).unwrap(),
    ];
    let board_u_b = vec![parse_unit("3/2", &defaults).unwrap()];
    let res_u = simulate(&board_u_a, &board_u_b, &GameState::default(), 1);
    assert_eq!(res_u.outcome, BattleOutcome::AWin);
    // Tentacle (1/2) and the 1/5 (now 2/5) survive.
    assert_eq!(res_u.survivors_a.len(), 2);
    assert_eq!(res_u.survivors_a[0].attack, 1);
    assert_eq!(res_u.survivors_a[0].health, 2);
    assert!(res_u.survivors_a[0].taunt);
    assert_eq!(res_u.survivors_a[1].attack, 2);

    // 3. Eternal Knight aura increments mid-combat when a friendly Eternal Knight dies.
    let board_ek_a = vec![
        parse_unit("4/2 card:eternal_knight", &defaults).unwrap(),
        parse_unit("4/2 card:eternal_knight", &defaults).unwrap(),
    ];
    let board_ek_b = vec![parse_unit("3/4", &defaults).unwrap()];
    let res_ek = simulate(&board_ek_a, &board_ek_b, &GameState::default(), 1);
    assert_eq!(res_ek.outcome, BattleOutcome::AWin);
    assert_eq!(res_ek.eternal_knights_died_a, 1);
    // Second Eternal Knight grew from 4/2 to 8/4 after the first died!
    assert_eq!(res_ek.survivors_a[0].attack, 8);
    assert!(res_ek.survivors_a[0].health > 0);
}

#[test]
fn tier2_tavern_minions_and_spells_work_end_to_end() {
    let templates = full_catalog();
    let mut pool = CardPool::new(templates);
    let mut rng = Rng::new(777);
    let mut state = TavernState::new().with_shop_spells(true);
    state.tavern_tier = 2;
    state.start_turn(&mut pool, &mut rng);
    state.gold = 20;

    // Shop has 4 minions + 1 Tavern Spell = 5 items when include_shop_spells is true on Tier 2.
    assert_eq!(state.shop.len(), 5);
    assert!(state.shop.iter().any(|u| u.is_spell));

    // 1. Volumizers + Mechagnome Interpreter
    state.add_to_hand(tier2::mechagnome_interpreter::template().instantiate());
    state.add_to_hand(tier2::red_volumizer::template().instantiate());
    state.add_to_hand(tier2::blue_volumizer::template().instantiate());
    state.add_to_hand(tier2::green_volumizer::template().instantiate());

    // Play Mechagnome Interpreter at pos 0 (3/1 Mech)
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
    // Play Red Volumizer standalone at pos 1 -> Volumizers gain +3/+0 (so Red is 6/1),
    // and Mechagnome Interpreter gives it +3/+1 -> 9/2!
    // Meanwhile Blue Volumizer in hand becomes 4/3 and Green Volumizer becomes 5/2!
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 1,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.auras.volumizer_bonus_atk, 3);
    assert_eq!(state.board[1].attack, 9);
    assert_eq!(state.board[1].health, 2);

    // Magnetize Blue Volumizer onto Red Volumizer at pos 1 ->
    // Blue Volumizer triggers +0/+3 Volumizer aura (buffing Red on board +3 HP AND Blue itself +3 HP to 4/6),
    // fuses 4/6 onto Red (9+4=13, 5+6=11), and Mechagnome Interpreter buffs target by +3/+1 -> 16/12!
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 1,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.auras.volumizer_bonus_hp, 3);
    assert_eq!(state.board[1].attack, 16);
    assert_eq!(state.board[1].health, 12);

    // Play Green Volumizer standalone at pos 2 -> +1/+1 Volumizer aura
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 2,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.auras.volumizer_bonus_atk, 4);
    assert_eq!(state.auras.volumizer_bonus_hp, 4);

    // 2. Fire Baller & Snow Baller shared scaling
    state.add_to_hand(tier2::fire_baller::template().instantiate());
    state.add_to_hand(tier2::snow_baller::template().instantiate());
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
    // Sell Snow Baller (at pos 0) -> gives board +1 HP, baller_bonus = 1
    state
        .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.auras.baller_bonus, 1);
    // Sell Fire Baller (now at pos 0) -> gives board +2 Attack, baller_bonus = 2
    state
        .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.auras.baller_bonus, 2);

    // 3. Lurking Lionfish Activate (2g) -> replaces shop[0] with Fishbait for left-most Beast to attack
    state.board.clear();
    state.hand.clear();
    state.gold = 10;
    state.board.push(tier2::lurking_lionfish::template().instantiate());
    state
        .step(
            TavernAction::Activate {
                board_pos: 0,
                target_pos: Some(0),
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    // Lurking Lionfish (3/4 Beast) attacked 0/1 Fishbait, killed it, and gained +5/+5 -> 8/9!
    assert_eq!(state.board[0].attack, 8);
    assert_eq!(state.board[0].health, 9);

    // 4. Laboratory Assistant + Demon Fodder on Refresh
    state.add_to_hand(tier2::soul_rewinder::template().instantiate());
    state.add_to_hand(tier2::laboratory_assistant::template().instantiate());
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 1,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 2,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.auras.fodder_per_refresh, [1, 1, 1]);
    let total_demon_atk_before: i32 = state
        .board
        .iter()
        .filter(|u| u.tribe.matches(Tribe::Demon))
        .map(|u| u.attack)
        .sum();
    state
        .step(TavernAction::Refresh, &mut pool, &mut rng)
        .unwrap();
    let total_demon_atk_after: i32 = state
        .board
        .iter()
        .filter(|u| u.tribe.matches(Tribe::Demon))
        .map(|u| u.attack)
        .sum();
    assert_eq!(total_demon_atk_after - total_demon_atk_before, 2);
    assert_eq!(state.auras.fodder_per_refresh, [1, 1, 0]);

    // 5. Wandering Willbreaker -> sell gives 2 spells, casting 1 discards the other
    state.hand.clear();
    state.board.push(tier2::wandering_willbreaker::template().instantiate());
    let wb_pos = state.board.len() - 1;
    state
        .step(TavernAction::Sell { board_pos: wb_pos }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.hand.len(), 2);
    assert!(state.hand[0].willbreaker_group > 0);
    assert_eq!(
        state.hand[0].willbreaker_group,
        state.hand[1].willbreaker_group
    );
    // Cast the first spell -> the second Willbreaker spell is automatically discarded!
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
    // If the cast spell opened a Discover/Choose-One, resolve it.
    if state.discover_pending.is_some() {
        state
            .step(
                TavernAction::ChooseDiscover { option_index: 0 },
                &mut pool,
                &mut rng,
            )
            .unwrap();
    }
    assert!(
        !state.hand.iter().any(|c| c.willbreaker_group > 0),
        "remaining Willbreaker spell should have been discarded"
    );

    // 6. Bilgewater Breakout + Lockbox acceleration
    state.board.clear();
    state.hand.clear();
    for _ in 0..5 {
        state.add_to_hand(tier2::bilgewater_breakout::template().instantiate());
        let h_idx = state.hand.len() - 1;
        state
            .step(
                TavernAction::Play {
                    hand_index: h_idx,
                    board_pos: 0,
                },
                &mut pool,
                &mut rng,
            )
            .unwrap();
        state
            .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
            .unwrap();
    }
    // 1st Bilgewater Breakout gave Lockbox (5 turns left); next 4 accelerated it by 4 turns -> 1 turn left.
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tokens::SPELL_LOCKBOX);
    assert_eq!(state.hand[0].lockbox_turns_left, 1);
    // Starting next turn opens the Lockbox into a Golden typed minion!
    state.start_turn(&mut pool, &mut rng);
    assert_eq!(state.hand.len(), 1);
    assert!(!state.hand[0].is_spell);
    assert!(state.hand[0].is_golden);
    assert_ne!(state.hand[0].tribe, Tribe::None);

    // 7. Tarecgosa + Winner's Bread persistence across resolve_combat_against
    state.board.clear();
    state.hand.clear();
    state.board.push(tier2::tarecgosa::template().instantiate());
    state
        .board
        .push(tier2::electric_synthesizer::template().instantiate());
    state.add_to_hand(spells::spell_by_name("Winner's Bread").unwrap());
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
    // Tarecgosa is now 6/7 with 1 stack of Winner's Bread.
    assert_eq!(state.board[0].attack, 6);
    assert_eq!(state.board[0].health, 7);
    let opp = vec![Unit::new("Dummy", 1, 1)];
    let c_res = state.resolve_combat_against(&opp, 1, & Default::default(), &[], 123);
    assert_eq!(c_res.outcome, BattleOutcome::AWin);
    // Tarecgosa permanently kept the +1/+1 Start of Combat buff from Electric Synthesizer -> 7/8!
    assert_eq!(state.board[0].attack, 7);
    assert_eq!(state.board[0].health, 8);
    // Starting the next turn triggers Winner's Bread (+1 Blood Gem = +1/+1) -> 8/9!
    state.start_turn(&mut pool, &mut rng);
    assert_eq!(state.board[0].attack, 8);
    assert_eq!(state.board[0].health, 9);
}

#[test]
fn tier3_combat_mechanics_work_end_to_end() {
    let defaults = Defaults::default();

    // 1. Wildfire Elemental excess damage cleave (plain hits 1 adjacent, Golden hits both adjacent)
    let board_wf_a = vec![
        parse_unit("12/6 golden card:wildfire_elemental", &defaults).unwrap(),
    ];
    let board_wf_b = vec![
        parse_unit("1/4", &defaults).unwrap(),
        parse_unit("1/2 taunt", &defaults).unwrap(),
        parse_unit("1/4", &defaults).unwrap(),
    ];
    let res_wf = simulate(&board_wf_a, &board_wf_b, &GameState::default(), 1);
    assert_eq!(res_wf.outcome, BattleOutcome::AWin);
    // Single attack by Side::A killed the 1/2 Taunt and cleaved 10 excess damage to BOTH 1/4 neighbors!
    let attacks_a = res_wf
        .events
        .iter()
        .filter(|e| matches!(e, Event::AttackDeclared { side: Side::A, .. }))
        .count();
    assert_eq!(attacks_a, 1);

    // 2. Amber Guardian + Blue Whelp + Roaring Recruiter + Diremuck Forager
    let board_dr_a = vec![
        parse_unit("2/6 card:blue_whelp", &defaults).unwrap(),
        parse_unit("5/5 card:amber_guardian", &defaults).unwrap(),
        parse_unit("2/6 card:roaring_recruiter", &defaults).unwrap(),
        parse_unit("4/3 card:diremuck_forager", &defaults).unwrap(),
    ];
    let board_dr_b = vec![parse_unit("2/2", &defaults).unwrap()];
    let mut state_dr = GameState::default();
    state_dr
        .hand_a
        .push(Unit::new("Hand Murloc", 3, 3).with_tribe(Tribe::Murloc));
    let res_dr = simulate(&board_dr_a, &board_dr_b, &state_dr, 7);
    assert_eq!(res_dr.outcome, BattleOutcome::AWin);
    // Hand Murloc (3/3) was summoned onto the board by Diremuck Forager!
    assert_eq!(res_dr.survivors_a.len(), 5);
    assert!(res_dr.survivors_a.iter().any(|u| u.name == "Hand Murloc"));
    // Blue Whelp attacked first: Roaring Recruiter gave it +3/+1, and Blue Whelp Rally incremented spell_bonus_hp!
    assert_eq!(res_dr.auras_a.spell_bonus_hp, 1);

    // 3. Relentless Deflector (Taunt while Divine Shield; Avenge (3) gains Divine Shield + Taunt)
    let mut deflector = tier3::relentless_deflector::template().instantiate();
    seaglass::cards::sync_unit_auras(&mut deflector, &Default::default());
    assert!(!deflector.divine_shield);
    assert!(!deflector.taunt);
    for _ in 0..3 {
        tier3::relentless_deflector::on_friendly_death(&mut deflector);
    }
    assert!(deflector.divine_shield);
    assert!(deflector.taunt);
    deflector.divine_shield = false;
    seaglass::cards::sync_unit_auras(&mut deflector, &Default::default());
    assert!(!deflector.taunt);

    // 4. Devout Hellcaller + Tasty Lobster + Waveling persistent combat effects
    let mut t_state = TavernState::new();
    t_state.auras.deity.kind = DeityKind::CThun;
    t_state.board.push(tier3::malchezaar_prince_of_dance::template().instantiate()); // Friendly Demon attacks first
    t_state.board.push(tier3::devout_hellcaller::template().instantiate()); // 4/4 Demon
    t_state.board.push(tier3::tasty_lobster::template().instantiate());
    t_state.board.push(tier3::waveling::template().instantiate());
    let opp_board = vec![
        Unit::new("Enemy1", 6, 4).with_keyword(Keyword::Taunt),
        Unit::new("Enemy2", 6, 4).with_keyword(Keyword::Taunt),
        Unit::new("Enemy3", 6, 4).with_keyword(Keyword::Taunt),
    ];
    let c_res = t_state.resolve_combat_against(&opp_board, 2, &Default::default(), &[], 42);
    // Malchezaar (Demon) dealt damage, so Devout Hellcaller permanently gained +2/+2 on the Tavern board!
    assert_eq!(t_state.board[1].attack, 6);
    assert_eq!(t_state.board[1].health, 6);
    // Tasty Lobster and Waveling died and incremented persistent aura stacks!
    assert_eq!(c_res.auras_a.tasty_lobster_stacks, 1);
    assert_eq!(c_res.auras_a.waveling_stacks, 1);
}

#[test]
fn tier3_tavern_minions_and_spells_work_end_to_end() {
    let templates = full_catalog();
    let mut pool = CardPool::new(templates);
    let mut rng = Rng::new(2026);
    let mut state = TavernState::new().with_shop_spells(true);
    state.tavern_tier = 3;
    state.start_turn(&mut pool, &mut rng);
    state.gold = 30;

    // 1. Prosthetic Hand (Undead/Mech Magnetic) can magnetize onto an Undead OR a Mech!
    state.board.clear();
    state.hand.clear();
    state.board.push(tier1::risen_rider::template().instantiate()); // 2/1 Undead
    state.add_to_hand(tier3::prosthetic_hand::template().instantiate()); // 3/1 Undead/Mech Magnetic Reborn
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
    assert_eq!(state.board.len(), 1);
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 2);

    // 2. Accord-o-Tron Magnetized grants +1 Gold at Start of Turn
    state.board.clear();
    state.board.push(tier1::cord_puller::template().instantiate());
    state.add_to_hand(tier3::accord_o_tron::template().instantiate());
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
    assert_eq!(state.board[0].sot_gold_bonus, 1);

    // 3. Thorned Trailblazer combines both Choose-One effects on Fearless Foodie!
    state.board.push(tier3::thorned_trailblazer::template().instantiate());
    state.add_to_hand(tier3::fearless_foodie::template().instantiate());
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 2,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    // Both options fired without pausing for discover_pending:
    // Blood Gems gained +1/+1 AND 4 Blood Gems were added to hand!
    assert!(state.discover_pending.is_none());
    assert_eq!(state.auras.blood_gem_bonus_atk, 1);
    assert_eq!(state.auras.blood_gem_bonus_hp, 1);
    assert_eq!(state.hand.len(), 4);
    assert!(state
        .hand
        .iter()
        .all(|c| c.card_id == tokens::SPELL_BLOOD_GEM));

    // 4. Gem Confiscation: play 2 Blood Gems on neighbor, then cast Gem Confiscation on target to steal them!
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
    assert_eq!(state.board[0].blood_gems_played, 2);
    assert_eq!(state.board[0].blood_gem_stats_applied, (4, 4));
    state.hand.clear();
    state.add_to_hand(tokens::make_gem_confiscation());
    let pre_tb_atk = state.board[1].attack;
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 1,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    // board[1] played 3 Blood Gems (+6/+6) AND stole 2 Blood Gems (+4/+4) from board[0] = +10/+10!
    assert_eq!(state.board[1].attack - pre_tb_atk, 10);
    assert_eq!(state.board[0].blood_gems_played, 0);

    // 5. Fetid Corroder + Abyssal Envoy: discarding Sludge Corrosion casts it twice AND generates a random Tavern spell!
    state.board.clear();
    state.hand.clear();
    state.add_to_hand(tier3::fetid_corroder::template().instantiate());
    state.add_to_hand(tier3::abyssal_envoy::template().instantiate());
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
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 1,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    // Hand now has Sludge Corrosion at index 0. Activate Abyssal Envoy (board[1]) targeting hand[0]!
    assert_eq!(state.hand[0].card_id, tokens::SPELL_SLUDGE_CORROSION);
    let pre_corroder_atk = state.board[0].attack;
    state
        .step(
            TavernAction::Activate {
                board_pos: 1,
                target_pos: Some(0),
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    // Abyssal Envoy replaced the discarded card with a random Tavern spell in hand!
    assert_eq!(state.hand.len(), 1);
    assert!(state.hand[0].is_spell);
    // Sludge Corrosion cast twice when discarded (+1/+1 twice = +2/+2)!
    assert_eq!(state.board[0].attack - pre_corroder_atk, 2);

    // 6. Malchezaar, Prince of Dance health refresh + Soul Rewinder
    state.board.clear();
    state.hand.clear();
    state.board.push(tier2::soul_rewinder::template().instantiate());
    state
        .board
        .push(tier3::malchezaar_prince_of_dance::template().instantiate());
    state.gold = 0;
    let pre_hp = state.health;
    // Even with 0 Gold, Refresh is legal because Malchezaar has 2 health refreshes!
    state
        .step(TavernAction::Refresh, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.health, pre_hp); // Rewound by Soul Rewinder!
    assert_eq!(state.board[0].health, 4); // Soul Rewinder gained +1 Health (3 -> 4)!
    assert_eq!(state.board[1].charges, 1);
}

#[test]
fn tier4_catalog_contains_all_58_live_solo_minions_and_16_spells() {
    let cards = tier4_catalog();
    assert_eq!(cards.len(), 58);
    assert_eq!(full_catalog().len(), 252);
    assert_eq!(catalog_for("tier4").unwrap().len(), 58);

    for (idx, card) in cards.iter().enumerate() {
        assert_eq!(card.tavern_tier, 4);
        assert!(card.card_id >= 401 && card.card_id <= 458);
        for other in &cards[idx + 1..] {
            assert_ne!(card.card_id, other.card_id);
        }
    }

    assert_eq!(spells::tier4_spells().len(), 16);
    assert_eq!(spells::spells_up_to_tier(4).len(), 46);
}

#[test]
fn tier5_catalog_contains_all_52_live_solo_minions_and_15_spells() {
    let cards = tier5_catalog();
    assert_eq!(cards.len(), 52);
    assert_eq!(full_catalog().len(), 252);
    assert_eq!(catalog_for("tier5").unwrap().len(), 52);

    for (idx, card) in cards.iter().enumerate() {
        assert_eq!(card.tavern_tier, 5);
        assert!(card.card_id >= 501 && card.card_id <= 552);
        for other in &cards[idx + 1..] {
            assert_ne!(card.card_id, other.card_id);
        }
    }

    assert_eq!(spells::tier5_spells().len(), 15);
    assert_eq!(spells::spells_up_to_tier(5).len(), 61);
}

#[test]
fn tier6_catalog_contains_all_32_live_solo_minions_and_5_spells() {
    let cards = tier6_catalog();
    assert_eq!(cards.len(), 32);
    assert_eq!(full_catalog().len(), 252);
    assert_eq!(catalog_for("tier6").unwrap().len(), 32);

    for (idx, card) in cards.iter().enumerate() {
        assert_eq!(card.tavern_tier, 6);
        assert!(card.card_id >= 601 && card.card_id <= 632);
        for other in &cards[idx + 1..] {
            assert_ne!(card.card_id, other.card_id);
        }
    }

    assert_eq!(spells::tier6_spells().len(), 5);
    assert_eq!(spells::spells_up_to_tier(6).len(), 66);
}

#[test]
fn tier7_catalog_contains_all_12_live_solo_minions_and_4_spells() {
    let cards = tier7_catalog();
    assert_eq!(cards.len(), 12);
    assert_eq!(full_catalog().len(), 252);
    assert_eq!(catalog_for("tier7").unwrap().len(), 12);

    for (idx, card) in cards.iter().enumerate() {
        assert_eq!(card.tavern_tier, 7);
        assert!(card.card_id >= 701 && card.card_id <= 712);
        for other in &cards[idx + 1..] {
            assert_ne!(card.card_id, other.card_id);
        }
    }

    assert_eq!(spells::tier7_spells().len(), 4);
    assert_eq!(spells::spells_up_to_tier(7).len(), 70);
}

#[test]
fn audit_regression_suite_covers_all_15_bugs() {
    use seaglass::cards::{deities, tier4, tier6};

    // 1. Deity CardId collision fixed & spell pool excludes token spells and Unmasked Identity
    assert_ne!(deities::CARD_CTHUN, spells::SPELL_A_NEW_SPROUT);
    assert_ne!(deities::CARD_YSHAARJ, spells::SPELL_ALLIANCE_FLAG);
    let mut rng = Rng::new(12345);
    for _ in 0..200 {
        let s = spells::draw_random_tavern_spell(6, &mut rng);
        assert!(!matches!(
            s.card_id,
            tokens::SPELL_GEM_CONFISCATION
                | tokens::SPELL_SLUDGE_CORROSION
                | tokens::SPELL_GOLDEN_TOUCH
                | spells::SPELL_UNMASKED_IDENTITY
        ));
    }

    // 2. Laboratory Assistant start-of-turn refresh spawns Demon Fodder
    let mut pool = CardPool::new(full_catalog());
    let mut state = TavernState::new();
    state.start_turn(&mut pool, &mut rng);
    state.gold = 10;
    state.add_to_hand(tier2::laboratory_assistant::template().instantiate());
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
    assert_eq!(state.auras.fodder_per_refresh, [1, 1, 1]);
    // With a friendly Demon (the Assistant itself) on board, the Fodder is absorbed by it.
    let (atk_before, hp_before) = (state.board[0].attack, state.board[0].health);
    state.start_turn(&mut pool, &mut rng);
    assert_eq!(state.auras.fodder_per_refresh, [1, 1, 0]);
    assert!(
        state.board[0].attack > atk_before && state.board[0].health > hp_before,
        "Fodder should buff the only friendly Demon"
    );
    // With no friendly Demon, the Fodder appears in the shop on the next start-of-turn roll.
    state.board.clear();
    state.start_turn(&mut pool, &mut rng);
    assert_eq!(state.auras.fodder_per_refresh, [1, 0, 0]);
    assert!(state
        .shop
        .iter()
        .any(|u| u.card_id == tokens::TOKEN_DEMON_FODDER));

    // 3. Mind Muck, Sprightly Scarab, and Lovesick Balladist do not self-target when no matching tribe on board
    state.board.clear();
    state.hand.clear();
    state.gold = 10;
    state.add_to_hand(tier2::mind_muck::template().instantiate());
    let shop_len_before = state.shop.len();
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
    assert_eq!(state.board[0].attack, 3);
    assert_eq!(state.board[0].health, 2);
    assert_eq!(state.shop.len(), shop_len_before);

    state.board.clear();
    state.add_to_hand(tier3::sprightly_scarab::template().instantiate());
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
    assert!(state.discover_pending.is_none());
    assert_eq!(state.board[0].attack, 3);
    assert_eq!(state.board[0].health, 1);

    state.board.clear();
    state.add_to_hand(tier4::lovesick_balladist::template().instantiate());
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
    assert_eq!(state.board[0].health, 4);

    // 4. Kelp Keeper preserves existing buffs on target Battlecry minion & filters valid targets
    state.board.clear();
    let mut geomancer = tier1::razorfen_geomancer::template().instantiate();
    geomancer.add_stats(10, 10);
    state.board.push(geomancer);
    state.board.push(tier4::kelp_keeper::template().instantiate());
    state.board.push(Unit::new("Vanilla", 2, 2));
    assert!(state.is_legal(&TavernAction::Activate {
        board_pos: 1,
        target_pos: Some(0),
    }));
    assert!(!state.is_legal(&TavernAction::Activate {
        board_pos: 1,
        target_pos: Some(2),
    }));
    state
        .step(
            TavernAction::Activate {
                board_pos: 1,
                target_pos: Some(0),
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    // Razorfen Geomancer is 2/1 base; the +10/+10 buff must survive the re-triggered Battlecry.
    assert_eq!(state.board[0].attack, 12);
    assert_eq!(state.board[0].health, 11);
    assert_eq!(state.hand.len(), 2);
    assert!(state
        .hand
        .iter()
        .all(|c| c.card_id == tokens::SPELL_BLOOD_GEM));
    state.hand.clear();

    // 5. Ashen Corruptor + Malchezaar health refresh buffs newly rolled shop and persists for the turn
    state.board.clear();
    state.board.push(tier4::ashen_corruptor::template().instantiate());
    state
        .board
        .push(tier3::malchezaar_prince_of_dance::template().instantiate());
    state.auras.ashen_corruptor_turn_buff = 0;
    state.gold = 10;
    state
        .step(TavernAction::Refresh, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.auras.ashen_corruptor_turn_buff, 2);
    for u in state.shop.iter().filter(|u| !u.is_spell) {
        assert!(u.attack >= u.base_attack + 2);
        assert!(u.health >= u.base_health + 2);
    }

    // 6. Gold above 10 is preserved across start_turn, Sell, and Gold spells; Armor absorbs tavern hero damage first
    state.board.clear();
    state.hand.clear();
    state.bonus_gold_next_turn = 3;
    // `start_turn` derives `max_gold` from the turn number; turn 9+ reaches the 10-Gold cap.
    state.turn = 9;
    state.start_turn(&mut pool, &mut rng);
    assert_eq!(state.max_gold, 10);
    assert_eq!(state.gold, 13);
    state.board.push(Unit::new("Dummy", 1, 1));
    state
        .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.gold, 14);
    state.health = 30;
    state.armor = 5;
    state.deal_hero_damage(3);
    assert_eq!(state.health, 30);
    assert_eq!(state.armor, 2);

    // 7. Tripling does not triple global auras and resets Golden Volumizer threshold_triggered
    state.board.clear();
    state.hand.clear();
    state.auras.volumizer_bonus_atk = 0;
    state.auras.volumizer_bonus_hp = 0;
    for i in 0..3 {
        state.add_to_hand(tier2::green_volumizer::template().instantiate());
        if i < 2 {
            state
                .step(
                    TavernAction::Play {
                        hand_index: 0,
                        board_pos: i,
                    },
                    &mut pool,
                    &mut rng,
                )
                .unwrap();
        }
    }
    assert_eq!(state.hand.len(), 1);
    assert!(state.hand[0].is_golden);
    assert!(!state.hand[0].threshold_triggered);
    // Golden Green Volumizer is 6/6 base, +2/+2 from the 2 Volumizer stacks = 8/8
    // (the aura must not be counted once per merged copy, which would give 12/12).
    assert_eq!(state.hand[0].attack, 8);
    assert_eq!(state.hand[0].health, 8);

    // 8. Tarecgosa retains Divine Shield gained in combat even when popped in combat
    let pre_combat = tier2::tarecgosa::template().instantiate();
    let mut survivor = pre_combat.clone();
    survivor.apply_keyword(Keyword::DivineShield, true); // gained in combat...
    survivor.divine_shield = false; // ...and popped before combat ended
    let mut tavern_unit = pre_combat.clone();
    tier2::tarecgosa::apply_post_combat_persistence(&pre_combat, &[survivor], &mut tavern_unit);
    assert!(tavern_unit.divine_shield);
    assert!(tavern_unit.inherent_divine_shield);

    // 9. Falling Sky Golem emits StatBuff when a Deathrattle minion dies, and AttackDeclared precedes Rally StatBuff
    let defaults = Defaults::default();
    let mut golem = tier6::falling_sky_golem::template().instantiate();
    golem.attack = 8;
    golem.health = 8;
    let board_a = vec![
        parse_unit("1/4 card:glim_guardian", &defaults).unwrap(),
        parse_unit("1/1 card:cord_puller", &defaults).unwrap(),
        golem,
    ];
    let board_b = vec![parse_unit("5/20 taunt", &defaults).unwrap()];
    let res = simulate(&board_a, &board_b, &GameState::default(), 7);
    let first_atk_idx = res
        .events
        .iter()
        .position(|e| matches!(e, Event::AttackDeclared { .. }))
        .unwrap();
    let first_rally_idx = res
        .events
        .iter()
        .position(|e| matches!(e, Event::StatBuff { reason: "Rally", .. }))
        .unwrap();
    assert!(first_atk_idx < first_rally_idx);
    assert!(res.events.iter().any(|e| matches!(
        e,
        Event::StatBuff {
            reason: "Death Aura",
            ..
        }
    )));
}

