//! Exhaustive per-card functional unit tests for all 43 Solo Tier 3 minions (Patch 36.6.3)
//! and all 5 Chromadrake sub-cards.

use seaglass::cards::{spells, tier1, tier2, tier3, tokens};
use seaglass::{
    full_catalog, simulate, BattleOutcome, CardPool, DeityKind, GameState, Rng, TavernAction,
    TavernState, Tribe, Unit,
};

fn setup_tavern(seed: u64) -> (TavernState, CardPool, Rng) {
    let pool = CardPool::new(full_catalog());
    let rng = Rng::new(seed);
    let mut state = TavernState::new().with_shop_spells(true);
    state.tavern_tier = 3;
    state.gold = 20;
    (state, pool, rng)
}

#[test]
fn card_301_abyssal_envoy() {
    let (mut state, mut pool, mut rng) = setup_tavern(301);
    state
        .board
        .push(tier3::abyssal_envoy::template().instantiate());
    state.add_to_hand(tokens::make_blood_gem());
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
    assert_eq!(state.hand.len(), 1);
    assert!(state.hand[0].is_spell);
    assert_ne!(state.hand[0].card_id, tokens::SPELL_BLOOD_GEM);
}

#[test]
fn card_302_accord_o_tron() {
    let (mut state, mut pool, mut rng) = setup_tavern(302);
    state.board.push(tier1::cord_puller::template().instantiate()); // 1/1 Mech
    state.add_to_hand(tier3::accord_o_tron::template().instantiate()); // 5/5 Magnetic (+1 Gold at SoT)
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
    state.start_turn(&mut pool, &mut rng);
    // Turn 1 base gold (3) + 1 from Accord-o-Tron = 4 Gold
    assert_eq!(state.gold, 4);
}

#[test]
fn card_303_amber_guardian() {
    let board_a = vec![
        tier1::glim_guardian::template().instantiate(),  // 1/4 Dragon
        tier3::amber_guardian::template().instantiate(), // 5/5 Dragon: SoC gives another Dragon +2/+2 and DS
    ];
    let board_b = vec![Unit::new("Enemy", 1, 1)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 303);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Glim Guardian gained +2/+2 & Divine Shield at SoC (plus +2 Atk on Rally -> 5/6)!
    assert_eq!(res.survivors_a[0].health, 6);
}

#[test]
fn card_304_annoy_o_module() {
    let (mut state, mut pool, mut rng) = setup_tavern(304);
    state
        .board
        .push(tier2::mechagnome_interpreter::template().instantiate()); // 3/1 Mech without DS or Taunt
    state.add_to_hand(tier3::annoy_o_module::template().instantiate()); // 2/4 Magnetic DS Taunt
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
    assert!(state.board[0].divine_shield);
    assert!(state.board[0].taunt);
}

#[test]
fn card_305_auto_accelerator() {
    let (mut state, mut pool, mut rng) = setup_tavern(305);
    state.add_to_hand(tier3::auto_accelerator::template().instantiate());
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
    assert_eq!(state.hand.len(), 1);
    assert!(matches!(
        state.hand[0].card_id,
        tier2::red_volumizer::ID | tier2::blue_volumizer::ID | tier2::green_volumizer::ID
    ));
}

#[test]
fn card_306_azsharan_cutlassier() {
    let (mut state, mut pool, mut rng) = setup_tavern(306);
    state.add_to_hand(tier3::azsharan_cutlassier::template().instantiate());
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
    assert_eq!(state.auras.spell_bonus_atk, 1);
}

#[test]
fn card_307_blue_whelp() {
    let board_a = vec![tier3::blue_whelp::template().instantiate()]; // 1/5 Rally: +1 spell_bonus_hp
    let board_b = vec![Unit::new("Enemy", 1, 1)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 307);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.auras_a.spell_bonus_hp, 1);
}

#[test]
fn card_308_cadaver_caretaker() {
    let board_a = vec![tier3::cadaver_caretaker::template().instantiate()]; // 3/3 DR: three 1/1 Skeletons
    let board_b = vec![Unit::new("Enemy", 3, 4)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 308);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Cadaver Caretaker dealt 3 dmg to 3/4, died, summoned three 1/1 Skeletons; first Skeleton killed the 3/1!
    assert_eq!(res.survivors_a.len(), 2);
}

#[test]
fn card_309_deadly_spore() {
    let spore = tier3::deadly_spore::template().instantiate();
    assert!(spore.venomous);
    let board_a = vec![spore]; // 1/1 Venomous
    let board_b = vec![Unit::new("Giant", 50, 50)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 309);
    assert_eq!(res.outcome, BattleOutcome::Draw);
}

#[test]
fn card_310_devout_hellcaller() {
    let (mut state, _pool, _rng) = setup_tavern(310);
    state
        .board
        .push(tier1::ominous_seer::template().instantiate()); // 2/1 Demon
    state
        .board
        .push(tier3::devout_hellcaller::template().instantiate()); // 4/4 Demon
    let opp = vec![Unit::new("Enemy", 1, 1)];
    state.resolve_combat_against(&opp, 1, &Default::default(), &[], 310);
    assert_eq!(state.board[1].attack, 6);
    assert_eq!(state.board[1].health, 6);
}

#[test]
fn card_311_diremuck_forager() {
    let mut gs = GameState::default();
    gs.hand_a
        .push(Unit::new("SmallMurloc", 2, 2).with_tribe(Tribe::Murloc));
    gs.hand_a
        .push(Unit::new("BigMurloc", 9, 9).with_tribe(Tribe::Murloc));
    let board_a = vec![tier3::diremuck_forager::template().instantiate()]; // 4/5
    let board_b = vec![Unit::new("Enemy", 1, 1)];
    let res = simulate(&board_a, &board_b, &gs, 311);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert!(res.survivors_a.iter().any(|u| u.name == "BigMurloc"));
}

#[test]
fn card_312_disguised_graverobber() {
    let (mut state, mut pool, mut rng) = setup_tavern(312);
    // Take 1 Eternal Knight out of the pool and place it on the board with buffs (+5/+5 and Taunt).
    pool.take_copy(tier2::eternal_knight::ID);
    assert_eq!(pool.remaining_copies(tier2::eternal_knight::ID), 14);
    let mut knight = tier2::eternal_knight::template().instantiate();
    knight.add_stats(5, 5);
    knight.taunt = true;
    state.board.push(knight);
    state.add_to_hand(tier3::disguised_graverobber::template().instantiate());
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
    // Eternal Knight was destroyed -> eternal_knights_died == 1, and a plain copy in hand is 8/4 without Taunt or the +5/+5 buff!
    // Pool count stays at 14 (1 returned when destroyed, 1 taken for the plain copy in hand).
    assert_eq!(state.auras.counter(tier2::eternal_knight::ID), 1);
    assert_eq!(pool.remaining_copies(tier2::eternal_knight::ID), 14);
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tier2::eternal_knight::ID);
    assert_eq!(state.hand[0].attack, 8);
    assert_eq!(state.hand[0].health, 4);
    assert!(!state.hand[0].taunt);

    // Destroying an Undead with Deathrattle + Reborn triggers its Deathrattle AND Reborns it on board!
    state.board.clear();
    state.hand.clear();
    state
        .board
        .push(tier2::scarlet_skull::template().instantiate()); // 2/1 Undead, Reborn, DR: Give a friendly Undead +1/+2
    state
        .board
        .push(tier1::risen_rider::template().instantiate()); // 2/1 Undead, Taunt, Reborn
    state.add_to_hand(tier3::disguised_graverobber::template().instantiate());
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
    // Scarlet Skull died -> Risen Rider gained +1/+2 (2/1 -> 3/3), Reborn Scarlet Skull is on board without Reborn, and plain copy in hand has Reborn!
    assert_eq!(state.board.len(), 3);
    assert_eq!(state.board[1].card_id, tier2::scarlet_skull::ID);
    assert!(!state.board[1].reborn);
    assert_eq!(state.board[2].card_id, tier1::risen_rider::ID);
    assert_eq!(state.board[2].attack, 3);
    assert_eq!(state.board[2].health, 3);
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tier2::scarlet_skull::ID);
    assert!(state.hand[0].reborn);

    // Destroying a Golden Undead token with granted Taunt produces a plain (non-Golden, unbuffed) token in hand!
    state.board.clear();
    state.hand.clear();
    let mut golden_hand = tokens::make_helping_hand(true, &state.auras); // 4/2 Golden Helping Hand with Reborn
    golden_hand.reborn = false; // Suppose it already lost Reborn and gained Taunt
    golden_hand.taunt = true;
    state.board.push(golden_hand);
    state.add_to_hand(tier3::disguised_graverobber::template().instantiate());
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
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tokens::TOKEN_HELPING_HAND);
    assert!(!state.hand[0].is_golden);
    assert_eq!(state.hand[0].attack, 2);
    assert_eq!(state.hand[0].health, 1);
    assert!(state.hand[0].reborn);
    assert!(!state.hand[0].taunt);
}

#[test]
fn card_313_drifting_sacrifice() {
    let mut gs = GameState::default();
    gs.auras_a.deity.kind = DeityKind::CThun;
    let board_a = vec![tier3::drifting_sacrifice::template().instantiate()]; // 2/1 Reborn DR: +2/+1 to Deity
    let board_b = vec![Unit::new("Enemy", 2, 5)];
    let res = simulate(&board_a, &board_b, &gs, 313);
    // Dies twice (initial + Reborn) -> +4/+2 to Deity!
    assert_eq!(res.auras_a.deity.attack, 5);
    assert_eq!(res.auras_a.deity.health, 3);
}

#[test]
fn card_314_fearless_foodie() {
    let (mut state, mut pool, mut rng) = setup_tavern(314);
    state.add_to_hand(tier3::fearless_foodie::template().instantiate());
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
    // Option 0: Blood Gems give +1/+1 this game
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.auras.blood_gem_bonus_atk, 1);
    assert_eq!(state.auras.blood_gem_bonus_hp, 1);
}

#[test]
fn card_315_fetid_corroder() {
    let (mut state, mut pool, mut rng) = setup_tavern(315);
    state.add_to_hand(tier3::fetid_corroder::template().instantiate());
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
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tokens::SPELL_SLUDGE_CORROSION);
}

#[test]
fn card_316_fruit_vendor() {
    let (mut state, mut pool, mut rng) = setup_tavern(316);
    state
        .board
        .push(tier3::fruit_vendor::template().instantiate());
    let pre_gold = state.gold;
    state
        .step(
            TavernAction::Activate {
                board_pos: 0,
                target_pos: None,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.gold, pre_gold - 1);
    assert_eq!(state.hand.len(), 2);
    assert!(state
        .hand
        .iter()
        .all(|c| c.card_id == spells::SPELL_TAVERN_DISH_BANANA));
}

#[test]
fn card_317_gem_rat() {
    let (mut state, mut pool, mut rng) = setup_tavern(317);
    state.board.push(tier3::gem_rat::template().instantiate());
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tokens::SPELL_GEM_DAY);
}

#[test]
fn card_318_greedy_conniver() {
    let (mut state, mut pool, mut rng) = setup_tavern(318);
    // Plain Greedy Conniver does not trigger Discover on sell
    state
        .board
        .push(tier3::greedy_conniver::template().instantiate());
    state
        .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert!(state.discover_pending.is_none());

    // Golden Greedy Conniver triggers Discover on sell
    state.board.push(
        tier3::greedy_conniver::template()
            .instantiate()
            .with_golden(true),
    );
    state
        .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert!(state.discover_pending.is_some());
}

#[test]
fn card_319_handless_forsaken() {
    let board_a = vec![tier3::handless_forsaken::template().instantiate()]; // 2/1 DR: 2/1 Helping Hand with Reborn
    let board_b = vec![Unit::new("Enemy", 2, 4)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 319);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a.len(), 1);
    assert_eq!(res.survivors_a[0].card_id, tokens::TOKEN_HELPING_HAND);
}

#[test]
fn card_320_hired_mount_and_all_5_chromadrakes() {
    let (mut state, mut pool, mut rng) = setup_tavern(320);
    state
        .board
        .push(tier3::hired_mount::template().instantiate());
    let pre_gold = state.gold;
    state
        .step(
            TavernAction::Activate {
                board_pos: 0,
                target_pos: None,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.gold, pre_gold - 2);
    assert_eq!(state.hand.len(), 1);
    assert!(matches!(
        state.hand[0].card_id,
        tokens::TOKEN_BLUE_CHROMADRAKE
            | tokens::TOKEN_BLACK_CHROMADRAKE
            | tokens::TOKEN_GREEN_CHROMADRAKE
            | tokens::TOKEN_BRONZE_CHROMADRAKE
            | tokens::TOKEN_RED_CHROMADRAKE
    ));

    // Also test each of the 5 Chromadrakes' Battlecries directly!
    state.hand.clear();
    state.board.clear();
    state
        .board
        .push(tier1::glim_guardian::template().instantiate()); // 1/4 Dragon
    for cid in tokens::CHROMADRAKE_IDS {
        state.add_to_hand(tokens::make_chromadrake(cid, false));
        let idx = state.hand.len() - 1;
        state
            .step(
                TavernAction::Play {
                    hand_index: idx,
                    board_pos: 1,
                },
                &mut pool,
                &mut rng,
            )
            .unwrap();
    }
    // Glim Guardian (1/4) gained +1/+3 (Green) and +3/+1 (Bronze) -> 5/8!
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 8);
    // Red Chromadrake (+1 spell_bonus_atk) and Black Chromadrake (+1 spell_bonus_hp)!
    assert_eq!(state.auras.spell_bonus_atk, 1);
    assert_eq!(state.auras.spell_bonus_hp, 1);
    // Blue Chromadrake added a 2-Cost Tavern spell to hand!
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].spell_cost, 2);
}

#[test]
fn card_321_iron_groundskeeper() {
    let (mut state, mut pool, mut rng) = setup_tavern(321);
    state.add_to_hand(tier3::iron_groundskeeper::template().instantiate());
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
    assert_eq!(state.hand.len(), 2);
    assert!(state
        .hand
        .iter()
        .all(|c| c.card_id == spells::SPELL_FORTIFY));
}

#[test]
fn card_322_locked_up_mutineer() {
    let (mut state, _pool, _rng) = setup_tavern(322);
    state
        .board
        .push(tier3::locked_up_mutineer::template().instantiate());
    let opp = vec![Unit::new("Enemy", 5, 5)];
    state.resolve_combat_against(&opp, 1, &Default::default(), &[], 322);
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tokens::SPELL_LOCKBOX);
    assert_eq!(state.hand[0].lockbox_turns_left, 5);

    // Second combat death accelerates the existing Lockbox by 1 turn (5 -> 4)!
    state.resolve_combat_against(&opp, 1, &Default::default(), &[], 322);
    assert_eq!(state.hand[0].lockbox_turns_left, 4);
}

#[test]
fn card_323_malchezaar_prince_of_dance() {
    let (mut state, mut pool, mut rng) = setup_tavern(323);
    state.add_to_hand(tier3::malchezaar_prince_of_dance::template().instantiate());
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
    assert_eq!(state.board[0].charges, 2);
    state.gold = 0;
    let pre_hp = state.health;
    state
        .step(TavernAction::Refresh, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.health, pre_hp - 1);
    assert_eq!(state.board[0].charges, 1);
}

#[test]
fn card_324_mangled_bandit() {
    let (mut state, mut pool, mut rng) = setup_tavern(324);
    state
        .board
        .push(tier3::mangled_bandit::template().instantiate());
    state.add_to_hand(tokens::make_tavern_coin());
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
    assert_eq!(state.hand.len(), 3);
    assert!(state
        .hand
        .iter()
        .all(|c| c.card_id == tokens::SPELL_BLOOD_GEM));
}

#[test]
fn card_325_mummifier() {
    let board_a = vec![
        tier3::mummifier::template().instantiate(), // 5/2 DR: Give a different friendly Undead Reborn
        tier2::eternal_knight::template().instantiate(), // 4/2 Undead without Reborn
    ];
    let board_b = vec![Unit::new("Enemy", 5, 5)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 325);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Eternal Knight gained Reborn from Mummifier!
    assert!(res.survivors_a[0].reborn);
}

#[test]
fn card_326_prosthetic_hand() {
    let (mut state, mut pool, mut rng) = setup_tavern(326);
    // Magnetize onto an Undead
    state
        .board
        .push(tier2::eternal_knight::template().instantiate()); // 4/2 Undead
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
    assert_eq!(state.board[0].attack, 7);
    assert_eq!(state.board[0].health, 3);
    assert!(state.board[0].reborn);
}

#[test]
fn card_327_relentless_deflector() {
    let board_a = vec![
        tier1::harmless_bonehead::template().instantiate(), // 1/1 DR: two 1/1 Skeletons (3 total deaths!)
        tier3::relentless_deflector::template().instantiate(), // 5/4 Avenge (3): Gain Divine Shield + Taunt
    ];
    let board_b = vec![Unit::new("Enemy", 2, 10)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 327);
    assert_eq!(res.outcome, BattleOutcome::AWin);
}

#[test]
fn card_328_rescue_bot() {
    let board_a = vec![tier3::rescue_bot::template().instantiate()]; // 2/1 Taunt DR: Get a Repair Job
    let board_b = vec![Unit::new("Enemy", 2, 2)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 328);
    assert_eq!(res.outcome, BattleOutcome::Draw);
    assert_eq!(res.hand_a.len(), 1);
    assert_eq!(res.hand_a[0].card_id, spells::SPELL_REPAIR_JOB);
}

#[test]
fn card_329_roaring_recruiter() {
    let board_a = vec![
        tier1::glim_guardian::template().instantiate(), // 1/4 Dragon
        tier3::roaring_recruiter::template().instantiate(), // 2/8: Whenever another friendly Dragon attacks, give it +3/+1
    ];
    let board_b = vec![Unit::new("Enemy", 2, 5)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 329);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Glim Guardian (1/4) gained +2 Atk (Rally) + +3/+1 (Roaring Recruiter) = 6/5 -> killed 2/5 and survived at 6/3!
    assert_eq!(res.survivors_a[0].attack, 6);
    assert_eq!(res.survivors_a[0].health, 3);
}

#[test]
fn card_330_shoalfin_mystic() {
    let (mut state, mut pool, mut rng) = setup_tavern(330);
    state
        .board
        .push(tier3::shoalfin_mystic::template().instantiate());
    state
        .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.auras.spell_bonus_atk, 1);
    assert_eq!(state.auras.spell_bonus_hp, 1);
}

#[test]
fn card_331_sly_infiltrator() {
    let (mut state, mut pool, mut rng) = setup_tavern(331);
    state.add_to_hand(tier3::sly_infiltrator::template().instantiate());
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
    // Option 0: Gain 2 free Refreshes
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.auras.free_refreshes, 2);
}

#[test]
fn card_332_sprightly_scarab() {
    let (mut state, mut pool, mut rng) = setup_tavern(332);
    state
        .board
        .push(tier1::flittering_bat::template().instantiate()); // 1/4 Beast
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
    // Option 0: +1/+1 and Reborn to target Beast (now at board[1] after Scarab inserted at 0)
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.board[1].attack, 2);
    assert_eq!(state.board[1].health, 5);
    assert!(state.board[1].reborn);
}

#[test]
fn card_333_tasty_lobster() {
    let board_a = vec![
        tier3::tasty_lobster::template().instantiate(), // 2/1 Beast DR: Give a friendly Beast +2/+1 & improve future
        tier3::tasty_lobster::template().instantiate(), // 2/1 Beast
        tier1::flittering_bat::template().instantiate(), // 1/4 Beast
    ];
    let board_b = vec![Unit::new("Enemy", 4, 10)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 333);
    assert_eq!(res.auras_a.counter(tier3::tasty_lobster::ID), 2);
}

#[test]
fn card_334_thorned_trailblazer() {
    let (mut state, mut pool, mut rng) = setup_tavern(334);
    state.add_to_hand(tier3::thorned_trailblazer::template().instantiate());
    state.add_to_hand(tier3::sly_infiltrator::template().instantiate());
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
    // Both options of Sly Infiltrator fired automatically!
    assert_eq!(state.auras.free_refreshes, 2);
    assert_eq!(state.hand.len(), 3);
    assert_eq!(state.board[0].charges, 0);
}

#[test]
fn card_335_timecapn_hooktail() {
    let (mut state, mut pool, mut rng) = setup_tavern(335);
    state
        .board
        .push(tier3::timecapn_hooktail::template().instantiate()); // 1/4
    state.add_to_hand(spells::spell_by_name("Hasty Excavation").unwrap());
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
    assert_eq!(state.board[0].attack, 2);
}

#[test]
fn card_336_trapped_clapper() {
    let board_a = vec![tier3::trapped_clapper::template().instantiate()]; // 2/2 DR: Add a Fodder to next 3 Refreshes
    let board_b = vec![Unit::new("Enemy", 3, 3)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 336);
    assert_eq!(tokens::fodder_per_refresh(&res.auras_a), [1, 1, 1]);
}

#[test]
fn card_337_treasure_parrot() {
    let mut parrot = tier3::treasure_parrot::template().instantiate();
    parrot.attack = 35;
    let board_a = vec![parrot];
    let board_b = vec![Unit::new("Boss", 1, 40)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 337);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.hand_a.len(), 1);
    assert_eq!(res.hand_a[0].card_id, tokens::SPELL_GOLDEN_TOUCH);
}

#[test]
fn card_338_trench_fighter() {
    let (mut state, mut pool, mut rng) = setup_tavern(338);
    state
        .board
        .push(tier3::trench_fighter::template().instantiate());
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tokens::SPELL_GEM_CONFISCATION);
}

#[test]
fn card_339_unwilling_slacker() {
    let board_a = vec![tier3::unwilling_slacker::template().instantiate()]; // 3/2 DR: random 1-Cost Tavern spell
    let board_b = vec![Unit::new("Enemy", 3, 3)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 339);
    assert_eq!(res.hand_a.len(), 1);
    assert!(res.hand_a[0].is_spell);
    assert_eq!(res.hand_a[0].spell_cost, 1);
}

#[test]
fn card_340_vicious_mindslasher() {
    let (mut state, mut pool, mut rng) = setup_tavern(340);
    state.auras.deity.kind = DeityKind::CThun;
    state
        .board
        .push(tier3::vicious_mindslasher::template().instantiate()); // 1/2
    state.add_to_hand(spells::spell_by_name("Hasty Excavation").unwrap());
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
    assert_eq!(state.board[0].attack, 2);
    assert_eq!(state.board[0].health, 4);
    assert_eq!(state.auras.deity.attack, 2);
    assert_eq!(state.auras.deity.health, 3);
}

#[test]
fn card_341_waveling() {
    let (mut state, mut pool, mut rng) = setup_tavern(341);
    state.board.push(tier3::waveling::template().instantiate()); // 5/1
    let opp = vec![Unit::new("Enemy", 5, 5)];
    state.resolve_combat_against(&opp, 1, &Default::default(), &[], 341);
    assert_eq!(state.auras.refresh_random_buffs, vec![(4, 4)]);
    state
        .step(TavernAction::Refresh, &mut pool, &mut rng)
        .unwrap();
    // One minion in the shop gained +4/+4 over its base stats!
    assert!(state
        .shop
        .iter()
        .any(|u| !u.is_spell && u.attack == u.base_attack + 4 && u.health == u.base_health + 4));
}

#[test]
fn card_342_wildfire_elemental() {
    let board_a = vec![tier3::wildfire_elemental::template()
        .instantiate()
        .with_golden(true)]; // 12/6 Golden
    let board_b = vec![
        Unit::new("Left", 1, 4),
        Unit::new("Mid", 1, 2).with_keyword( seaglass::Keyword::Taunt),
        Unit::new("Right", 1, 4),
    ];
    let res = simulate(&board_a, &board_b, &GameState::default(), 342);
    assert_eq!(res.outcome, BattleOutcome::AWin);
}

#[test]
fn card_343_wolf_pup() {
    let board_a = vec![
        tier3::wolf_pup::template().instantiate(), // 3/6 Rally: Give your other minions +4/+1
        Unit::new("Ally", 1, 1),
    ];
    let board_b = vec![Unit::new("Enemy", 1, 1)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 343);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a[1].attack, 5);
    assert_eq!(res.survivors_a[1].health, 2);
}
