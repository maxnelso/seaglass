//! Per-card functional tests for all 58 Solo Tier 4 minions (Patch 36.6.3).

use seaglass::cards::{spells, tier1, tier2, tier4, tokens};
use seaglass::{
    full_catalog, simulate, BattleOutcome, CardPool, DeityKind, GameState, Keyword, PlayerAuras,
    Rng, TavernAction, TavernState, Tribe, Unit,
};

fn setup_tavern(seed: u64) -> (TavernState, CardPool, Rng) {
    let mut state = TavernState::new();
    state.gold = 10;
    state.tavern_tier = 4;
    let pool = CardPool::new(full_catalog());
    let rng = Rng::new(seed);
    (state, pool, rng)
}

fn run_combat(state: &mut TavernState, opponent_board: &[Unit], seed: u64) {
    state.resolve_combat_against(opponent_board, 1, &PlayerAuras::default(), &[], seed);
}

#[test]
fn card_401_air_baller() {
    let (mut state, mut pool, mut rng) = setup_tavern(401);
    state.board.push(Unit::new("Watcher", 1, 1));
    state.board.push(tier4::air_baller::template().instantiate());
    state
        .step(TavernAction::Sell { board_pos: 1 }, &mut pool, &mut rng)
        .unwrap();
    // Air Baller gives +2/+2 (plus baller_bonus=0) and increments baller_bonus by 1.
    assert_eq!(state.board[0].attack, 3);
    assert_eq!(state.board[0].health, 3);
    assert_eq!(state.auras.baller_bonus, 1);
}

#[test]
fn card_402_ashen_corruptor() {
    let (mut state, mut pool, mut rng) = setup_tavern(402);
    state
        .board
        .push(tier4::ashen_corruptor::template().instantiate());
    state.shop.clear();
    state.shop.push(Unit::new("ShopMinion", 2, 2));
    state
        .shop
        .push(spells::spell_by_name("Hasty Excavation").unwrap());
    state
        .step(TavernAction::Buy { shop_index: 1 }, &mut pool, &mut rng)
        .unwrap();
    // Hero damage from buying Hasty Excavation is rewound (stays 30 HP) and shop minions gain +2/+2!
    assert_eq!(state.health, 30);
    assert_eq!(state.shop[0].attack, 4);
    assert_eq!(state.shop[0].health, 4);
}

#[test]
fn card_403_banana_slamma() {
    let a = vec![
        tier1::buzzing_vermin::template().instantiate(), // 1/1 Taunt, Deathrattle: Summon 2/2 Beetle
        tier4::banana_slamma::template().instantiate(),  // Doubles Beast Attack summoned in combat
    ];
    let b = vec![Unit::new("KillVermin", 1, 1)];
    let res = simulate(&a, &b, &GameState::default(), 403);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    let beetle = res
        .survivors_a
        .iter()
        .find(|u| u.name == "Beetle")
        .expect("Beetle summoned");
    assert_eq!(beetle.attack, 4);
    assert_eq!(beetle.health, 2);
}

#[test]
fn card_404_bigwig_bandit() {
    let (mut state, _pool, _rng) = setup_tavern(404);
    state
        .board
        .push(tier4::bigwig_bandit::template().instantiate());
    run_combat(&mut state, &[Unit::new("Target", 0, 4)], 404);
    assert_eq!(state.hand.len(), 1);
    assert!(spells::BOUNTY_SPELL_IDS.contains(&state.hand[0].card_id));
}

#[test]
fn card_405_blade_collector() {
    let a = vec![tier4::blade_collector::template().instantiate()]; // 3/2 Cleave
    let b = vec![
        Unit::new("Left", 1, 3),
        Unit::new("Center", 1, 3).with_keyword(Keyword::Taunt),
        Unit::new("Right", 1, 3),
    ];
    let res = simulate(&a, &b, &GameState::default(), 405);
    assert_eq!(res.outcome, BattleOutcome::Draw);
    assert!(res.survivors_b.is_empty());
}

#[test]
fn card_406_bonker() {
    let a = vec![
        tier4::bonker::template().instantiate(), // 2/7 Windfury, Rally: Play a Blood Gem on all other minions
        Unit::new("Ally", 1, 10),
    ];
    let b = vec![Unit::new("Dummy", 0, 4)];
    let res = simulate(&a, &b, &GameState::default(), 406);
    let ally = res
        .survivors_a
        .iter()
        .find(|u| u.name == "Ally")
        .expect("Ally survives");
    // Bonker attacks twice with Windfury (2 Rally triggers = +2/+2)
    assert_eq!(ally.attack, 3);
    assert_eq!(ally.health, 12);
}

#[test]
fn card_407_boom_in_a_box() {
    let a = vec![tier4::boom_in_a_box::template().instantiate()]; // 5/10 Taunt, SoC: 3 dmg to all other minions
    let b = vec![Unit::new("Fragile1", 10, 3), Unit::new("Fragile2", 10, 3)];
    let res = simulate(&a, &b, &GameState::default(), 407);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a.len(), 1);
    assert_eq!(res.survivors_a[0].health, 10);
}

#[test]
fn card_408_bramble_tunneler() {
    let (mut state, _pool, _rng) = setup_tavern(408);
    state
        .board
        .push(tier4::bramble_tunneler::template().instantiate());
    run_combat(&mut state, &[Unit::new("Target", 1, 1)], 408);
    assert_eq!(state.hand.len(), 1);
}

#[test]
fn card_409_bream_counter() {
    let (mut state, mut pool, mut rng) = setup_tavern(409);
    state.add_to_hand(tier4::bream_counter::template().instantiate()); // 6/6 in hand
    state.add_to_hand(tier1::flighty_scout::template().instantiate()); // Murloc
    state
        .step(
            TavernAction::Play {
                hand_index: 1,
                board_pos: 0,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.hand[0].attack, 12);
    assert_eq!(state.hand[0].health, 12);
}

#[test]
fn card_410_bronze_timewalker() {
    let (mut state, _pool, _rng) = setup_tavern(410);
    state
        .board
        .push(tier4::bronze_timewalker::template().instantiate());
    run_combat(&mut state, &[Unit::new("Target", 1, 1)], 410);
    assert_eq!(state.hand.len(), 1);
    assert!(tokens::CHROMADRAKE_IDS.contains(&state.hand[0].card_id));
}

#[test]
fn card_411_cage_gnawer() {
    let a = vec![
        Unit::new("AttackerBeast", 2, 10).with_tribe(Tribe::Beast),
        tier4::cage_gnawer::template().instantiate(), // 2/7 Beast: whenever Beast attacks, Beasts gain +2/+1
    ];
    let b = vec![Unit::new("Target", 1, 5)];
    let res = simulate(&a, &b, &GameState::default(), 411);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert!(res.survivors_a[0].attack >= 4);
    assert!(res.survivors_a[1].attack >= 4);
}

#[test]
fn card_412_conveyor_construct() {
    let (mut state, _pool, _rng) = setup_tavern(412);
    state
        .board
        .push(tier4::conveyor_construct::template().instantiate());
    run_combat(&mut state, &[Unit::new("Killer", 10, 10)], 412);
    assert_eq!(state.hand.len(), 1);
    assert!(matches!(
        state.hand[0].card_id,
        tier2::blue_volumizer::ID | tier2::green_volumizer::ID | tier2::red_volumizer::ID
    ));
}

#[test]
fn card_413_cutthroat_kthir() {
    let (mut state, mut pool, mut rng) = setup_tavern(413);
    state.auras.deity.kind = DeityKind::CThun;
    state
        .board
        .push(tier2::brain_rotter::template().instantiate());
    state
        .board
        .push(tier4::cutthroat_kthir::template().instantiate()); // 4/4
    state.add_to_hand(Unit::new("Fodder", 1, 1));
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
    assert_eq!(state.board[1].attack, 8);
    assert_eq!(state.board[1].health, 8);
    // Brain Rotter gave +2/+2 and Cutthroat K'Thir gave +4/+4 -> Deity has +6/+6 (total 7/7)
    assert_eq!(state.auras.deity.attack, 7);
    assert_eq!(state.auras.deity.health, 7);
}

#[test]
fn card_414_dark_paradox() {
    let (mut state, _pool, _rng) = setup_tavern(414);
    state
        .board
        .push(tier4::dark_paradox::template().instantiate());
    state
        .board
        .push(Unit::new("Beast1", 2, 2).with_tribe(Tribe::Beast));
    state
        .board
        .push(Unit::new("Beast2", 2, 2).with_tribe(Tribe::Beast));
    run_combat(&mut state, &[Unit::new("Target", 0, 1)], 414);
    assert_eq!(state.hand.len(), 1);
    assert!(state.hand[0].tribe.matches(Tribe::Beast));
}

#[test]
fn card_415_dead_bellringer() {
    let (mut state, mut pool, mut rng) = setup_tavern(415);
    state
        .board
        .push(tier4::dead_bellringer::template().instantiate()); // 3/6
    state
        .board
        .push(tier1::harmless_bonehead::template().instantiate()); // 1/1 Undead
    state
        .step(
            TavernAction::Activate {
                board_pos: 0,
                target_pos: Some(1),
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    // Dead Bellringer gains +4/+4 (to 7/10), gives Harmless Bonehead Reborn, then destroys it:
    // Harmless Bonehead summons two 1/1 Skeletons + Reborn 1/1 Harmless Bonehead!
    assert_eq!(state.board[0].attack, 7);
    assert_eq!(state.board[0].health, 10);
    assert_eq!(state.board.len(), 4);
    assert!(state.board.iter().any(|u| u.name == "Skeleton"));
}

#[test]
fn card_416_drone_duplicator() {
    let (mut state, mut pool, mut rng) = setup_tavern(416);
    state
        .board
        .push(tier4::drone_duplicator::template().instantiate()); // 5/2 Mech
    state.add_to_hand(tier1::lullabot::template().instantiate()); // 2/2 Magnetic (+1 EOT HP)
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
    // Magnetized twice: +4/+4 stats (to 9/6) and +2 eot_health_bonus!
    assert_eq!(state.board[0].attack, 9);
    assert_eq!(state.board[0].health, 6);
    assert_eq!(state.board[0].eot_health_bonus, 2);
}

#[test]
fn card_417_en_djinn_blazer() {
    let (mut state, mut pool, mut rng) = setup_tavern(417);
    state.add_to_hand(tier4::en_djinn_blazer::template().instantiate());
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
    assert_eq!(state.auras.refresh_random_buffs, vec![(10, 10)]);
    state
        .step(TavernAction::Refresh, &mut pool, &mut rng)
        .unwrap();
    assert!(state
        .shop
        .iter()
        .any(|u| u.attack >= u.base_attack + 10 && u.health >= u.base_health + 10));
}

#[test]
fn card_418_enchanted_sentinel() {
    let (mut state, mut pool, mut rng) = setup_tavern(418);
    state.add_to_hand(tier4::enchanted_sentinel::template().instantiate());
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
    assert_eq!(state.auras.spell_bonus_hp, 1);
    state.add_to_hand(spells::make_tavern_dish_banana()); // +2/+2 + 1/1 = +3/+3
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
    assert_eq!(state.board[0].attack, 6);
    assert_eq!(state.board[0].health, 8);
}

#[test]
fn card_419_faceless_operative() {
    let (mut state, mut pool, mut rng) = setup_tavern(419);
    state
        .board
        .push(tier4::faceless_operative::template().instantiate());
    state
        .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.hand.len(), 2);
    assert!(state.hand.iter().all(|u| u.tribe.matches(Tribe::Aberration)));
    // Playing one discards the other!
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
    assert!(state.hand.is_empty());
    assert_eq!(state.auras.cards_discarded, 1);
}

#[test]
fn card_420_flaming_enforcer() {
    let (mut state, mut pool, mut rng) = setup_tavern(420);
    state
        .board
        .push(tier4::flaming_enforcer::template().instantiate()); // 4/5
    state.shop.push(Unit::new("Small", 2, 2));
    state.shop.push(Unit::new("Big", 6, 9));
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.shop.len(), 1);
    assert_eq!(state.board[0].attack, 10);
    assert_eq!(state.board[0].health, 14);
}

#[test]
fn card_421_friendly_geist() {
    let (mut state, _pool, _rng) = setup_tavern(421);
    state
        .board
        .push(tier4::friendly_geist::template().instantiate());
    run_combat(&mut state, &[Unit::new("Killer", 10, 10)], 421);
    assert_eq!(state.auras.spell_bonus_atk, 1);
}

#[test]
fn card_422_gearfin() {
    let (mut state, mut pool, mut rng) = setup_tavern(422);
    state.board.push(tier4::gearfin::template().instantiate());
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.hand.len(), 2);
    assert!(state.hand.iter().all(|u| u.is_spell && u.spell_cost == 1));
}

#[test]
fn card_423_geomagus_roogug() {
    let (mut state, mut pool, mut rng) = setup_tavern(423);
    state
        .board
        .push(tier4::geomagus_roogug::template().instantiate()); // 4/6
    state.board.push(Unit::new("Ally", 2, 2));
    state.add_to_hand(tokens::make_blood_gem());
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
    // Roogug gained +1/+1 (5/7) and played a Blood Gem on Ally (3/3)!
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 7);
    assert_eq!(state.board[1].attack, 3);
    assert_eq!(state.board[1].health, 3);
}

#[test]
fn card_424_glambot() {
    let (mut state, mut pool, mut rng) = setup_tavern(424);
    state.board.push(tier4::glambot::template().instantiate()); // 4/4 Mech
    state.add_to_hand(spells::make_tavern_dish_banana()); // +2/+2 spell
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
    // Glambot gained +2/+2 from Banana AND +4/+4 from Magnetized Satellite -> 10/10!
    assert_eq!(state.board[0].attack, 10);
    assert_eq!(state.board[0].health, 10);
}

#[test]
fn card_425_gormling_gourmet() {
    let (mut state, mut pool, mut rng) = setup_tavern(425);
    state.add_to_hand(tier4::gormling_gourmet::template().instantiate());
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
    assert_eq!(state.hand[0].card_id, spells::SPELL_SEAFOOD_STEW);
    run_combat(&mut state, &[Unit::new("Killer", 10, 10)], 425);
    assert_eq!(state.hand.len(), 2);
}

#[test]
fn card_426_gunpowder_courier() {
    let (mut state, mut pool, mut rng) = setup_tavern(426);
    state
        .board
        .push(tier4::gunpowder_courier::template().instantiate()); // 2/5 Pirate
    state.shop.push(Unit::new("M1", 1, 1));
    state.shop.push(Unit::new("M2", 1, 1));
    state
        .step(TavernAction::Buy { shop_index: 0 }, &mut pool, &mut rng)
        .unwrap(); // 3g spent
    assert_eq!(state.board[0].attack, 2);
    state
        .step(TavernAction::Buy { shop_index: 0 }, &mut pool, &mut rng)
        .unwrap(); // 6g spent -> triggers +3/+1!
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 6);
}

#[test]
fn card_427_headhunter_gryphon() {
    let (mut state, _pool, _rng) = setup_tavern(427);
    state
        .board
        .push(tier4::headhunter_gryphon::template().instantiate());
    run_combat(&mut state, &[Unit::new("Target", 1, 1)], 427);
    assert_eq!(state.hand.len(), 1);
    assert!(state.hand[0].tribe.matches(Tribe::Beast));
}

#[test]
fn card_428_heroic_underdog() {
    let a = vec![
        tier4::heroic_underdog::template().instantiate(), // 1/10 Stealth, Rally: Gain target's Attack
        Unit::new("Spotter", 0, 1),                       // Ensures Side A has more minions and attacks first
    ];
    let b = vec![Unit::new("Giant", 9, 8)];
    let res = simulate(&a, &b, &GameState::default(), 428);
    // Heroic Underdog attacks first, gains +9 Attack (to 10/10) BEFORE combat damage, killing 9/8 Giant in 1 hit and surviving at 10/1!
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a[0].attack, 10);
    assert_eq!(res.survivors_a[0].health, 1);
}

#[test]
fn card_429_hoarding_hyena() {
    let a = vec![tier4::hoarding_hyena::template().instantiate()];
    let b = vec![Unit::new("Dummy", 1, 1)];
    let res = simulate(&a, &b, &GameState::default(), 429);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert!(res.survivors_a.iter().any(|u| u.name == "Tasty Lobster"));
}

#[test]
fn card_430_holy_vanguard() {
    let (mut state, mut pool, mut rng) = setup_tavern(430);
    state.add_to_hand(tier4::holy_vanguard::template().instantiate());
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
    assert_eq!(state.board[0].attack, 10);
    assert_eq!(state.board[0].health, 10);
    state.deal_hero_damage(15); // Hero health drops to 15
    assert_eq!(state.board[0].attack, 40);
    assert_eq!(state.board[0].health, 40);
}

#[test]
fn card_431_hot_air_surveyor() {
    let (mut state, mut pool, mut rng) = setup_tavern(431);
    state
        .board
        .push(tier4::hot_air_surveyor::template().instantiate()); // 3/7
    state.add_to_hand(tokens::make_blood_gem());
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
    // Blood Gem from hand casts twice (+2/+2 -> 5/9)!
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 9);
    assert_eq!(state.board[0].blood_gems_played, 2);
}

#[test]
fn card_432_humongozz() {
    let (mut state, mut pool, mut rng) = setup_tavern(432);
    state.add_to_hand(tier4::humongozz::template().instantiate()); // 5/5 Divine Shield, spells give +1/+2
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
    assert_eq!(state.auras.spell_bonus_hp, 2);
}

#[test]
fn card_433_ichoron_the_protector() {
    let (mut state, mut pool, mut rng) = setup_tavern(433);
    state
        .board
        .push(tier4::ichoron_the_protector::template().instantiate());
    state.add_to_hand(tier1::dune_dweller::template().instantiate()); // Elemental without DS
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
    assert!(state.board[1].divine_shield);
    assert!(state.board[1].temp_divine_shield);
    // Expires at start of next turn!
    state.start_turn(&mut pool, &mut rng);
    assert!(!state.board[1].divine_shield);
}

#[test]
fn card_434_imp_lusionist() {
    let (mut state, _pool, _rng) = setup_tavern(434);
    state
        .board
        .push(tier4::imp_lusionist::template().instantiate());
    run_combat(&mut state, &[Unit::new("Killer", 10, 10)], 434);
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, spells::SPELL_METHODICAL_MADNESS);
}

#[test]
fn card_435_imposing_percussionist() {
    let (mut state, mut pool, mut rng) = setup_tavern(435);
    state.add_to_hand(tier4::imposing_percussionist::template().instantiate());
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
    assert!(state.discover_pending.is_some());
    let chosen_tier = state.discover_pending.as_ref().unwrap()[0].tavern_tier as i32;
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.health, 30 - chosen_tier);
    assert_eq!(state.hand.len(), 1);
    assert!(state.hand[0].tribe.matches(Tribe::Demon));
}

#[test]
fn card_436_kelp_keeper() {
    let (mut state, mut pool, mut rng) = setup_tavern(436);
    state
        .board
        .push(tier4::kelp_keeper::template().instantiate());
    state
        .board
        .push(tier1::razorfen_geomancer::template().instantiate()); // Battlecry: Get 2 Blood Gems
    state
        .step(
            TavernAction::Activate {
                board_pos: 0,
                target_pos: Some(1),
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.hand.len(), 2);
    assert!(state.hand.iter().all(|u| u.card_id == tokens::SPELL_BLOOD_GEM));
}

#[test]
fn card_437_leyline_surfacer() {
    let (mut state, mut pool, mut rng) = setup_tavern(437);
    state.add_to_hand(tier4::leyline_surfacer::template().instantiate());
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
    assert_eq!(state.hand[0].card_id, tokens::SPELL_ARCANE_ABSORPTION);
    run_combat(&mut state, &[Unit::new("Killer", 10, 10)], 437);
    assert_eq!(state.hand.len(), 2);
}

#[test]
fn card_438_living_prison() {
    let (mut state, mut pool, mut rng) = setup_tavern(438);
    state
        .board
        .push(tier4::living_prison::template().instantiate()); // 4/5
    state.shop.push(Unit::new("ShopGiant", 6, 7));
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
    state
        .step(TavernAction::Buy { shop_index: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.board[0].attack, 10);
    assert_eq!(state.board[0].health, 12);
}

#[test]
fn card_439_lovesick_balladist() {
    let (mut state, mut pool, mut rng) = setup_tavern(439);
    state
        .board
        .push(Unit::new("PirateAlly", 2, 2).with_tribe(Tribe::Pirate));
    state.shop.push(tier4::lovesick_balladist::template().instantiate());
    // Buying costs 3 Gold -> gold_spent_this_turn = 3 -> gives +2 + 3 = +5 Health!
    state
        .step(TavernAction::Buy { shop_index: 0 }, &mut pool, &mut rng)
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
    assert_eq!(state.board[0].health, 7);
}

#[test]
fn card_440_maritime_extortionist() {
    let (mut state, mut pool, mut rng) = setup_tavern(440);
    state
        .board
        .push(tier4::maritime_extortionist::template().instantiate()); // 7/7
    state
        .add_to_hand(tier1::aureate_laureate::template().instantiate()); // Golden minion!
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
    assert_eq!(state.auras.golden_minions_played, 1);
    assert_eq!(state.board[0].attack, 14);
    assert_eq!(state.board[0].health, 14);
}

#[test]
fn card_441_maw_caster() {
    let (mut state, mut pool, mut rng) = setup_tavern(441);
    state
        .board
        .push(tier1::risen_rider::template().instantiate()); // Undead
    state.add_to_hand(tier4::maw_caster::template().instantiate());
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
    assert!(state.discover_pending.is_some());
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.hand.len(), 1);
    assert!(state.hand[0].tribe.matches(Tribe::Undead));
}

#[test]
fn card_442_mindbending_recruiter() {
    let (mut state, mut pool, mut rng) = setup_tavern(442);
    state
        .board
        .push(tier4::mindbending_recruiter::template().instantiate());
    state.add_to_hand(Unit::new("Fodder", 1, 1));
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
    assert!(state.hand[0].tribe.matches(Tribe::Aberration));
}

#[test]
fn card_443_nightmare_corroder() {
    let (mut state, mut pool, mut rng) = setup_tavern(443);
    state
        .board
        .push(tier4::nightmare_corroder::template().instantiate());
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tokens::SPELL_SLUDGE_CORROSION);
}

#[test]
fn card_444_parasitic_fleshling() {
    let (mut state, mut pool, mut rng) = setup_tavern(444);
    state.auras.cards_discarded = 3;
    state
        .board
        .push(tier4::parasitic_fleshling::template().instantiate()); // 4/6
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    // Gives left-most minion +(2 + 3)/+(2 + 3) = +5/+5 -> 9/11!
    assert_eq!(state.board[0].attack, 9);
    assert_eq!(state.board[0].health, 11);
}

#[test]
fn card_445_persistent_poet() {
    let (mut state, _pool, _rng) = setup_tavern(445);
    state
        .board
        .push(tier2::electric_synthesizer::template().instantiate()); // SoC: Give other Dragons +2/+1
    state
        .board
        .push(tier4::persistent_poet::template().instantiate()); // Adjacent Persistent Poet
    state
        .board
        .push(Unit::new("OtherDragon", 2, 2).with_tribe(Tribe::Dragon)); // Adjacent Dragon
    run_combat(&mut state, &[Unit::new("Dummy", 1, 1)], 445);
    // Electric Synthesizer gave OtherDragon +1/+1 in combat, and Persistent Poet made it permanent!
    assert_eq!(state.board[2].attack, 3);
    assert_eq!(state.board[2].health, 3);
}

#[test]
fn card_446_plaguerunner() {
    let (mut state, _pool, _rng) = setup_tavern(446);
    state
        .board
        .push(tier4::plaguerunner::template().instantiate());
    run_combat(&mut state, &[Unit::new("Killer", 10, 10)], 446);
    // In combat: +2 Undead attack!
    assert_eq!(state.auras.undead_bonus_attack, 2);
}

#[test]
fn card_447_razorfen_flapper() {
    let (mut state, mut pool, mut rng) = setup_tavern(447);
    state.add_to_hand(tier4::razorfen_flapper::template().instantiate());
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
    assert_eq!(state.hand[0].card_id, spells::SPELL_BLOOD_GEM_BARRAGE);
    run_combat(&mut state, &[Unit::new("Killer", 10, 10)], 447);
    assert_eq!(state.hand.len(), 2);
}

#[test]
fn card_448_refreshing_anomaly() {
    let (mut state, mut pool, mut rng) = setup_tavern(448);
    state.add_to_hand(tier4::refreshing_anomaly::template().instantiate());
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
    assert_eq!(state.auras.free_refreshes, 2);
}

#[test]
fn card_449_runic_arcanist() {
    let a = vec![tier4::runic_arcanist::template().instantiate()]; // 2/4, SoC: Cast Shiny Ring twice (+2/+2 -> 4/6)
    let b = vec![Unit::new("Dummy", 2, 5)];
    let res = simulate(&a, &b, &GameState::default(), 449);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a[0].attack, 4);
    assert_eq!(res.survivors_a[0].health, 2);
}

#[test]
fn card_450_sacrificial_wrathguard() {
    let (mut state, mut pool, mut rng) = setup_tavern(450);
    state
        .board
        .push(tier4::sacrificial_wrathguard::template().instantiate());
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
    assert_eq!(state.board[0].stacks, 2);
    run_combat(&mut state, &[Unit::new("Killer", 10, 10)], 450);
    // Base +2/+2 + improved +2/+2 = +4/+4 to Tavern minions!
    assert_eq!(state.auras.tavern_all_atk, 4);
    assert_eq!(state.auras.tavern_all_hp, 4);
}

#[test]
fn card_451_sindorei_straight_shot() {
    let a = vec![tier4::sindorei_straight_shot::template().instantiate()]; // 3/4 DS Windfury, Rally: Remove Reborn & Taunt
    let b = vec![Unit::new("RebornTaunt", 2, 3)
        .with_keyword(Keyword::Taunt)
        .with_keyword(Keyword::Reborn)];
    let res = simulate(&a, &b, &GameState::default(), 451);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert!(res.survivors_b.is_empty());
}

#[test]
fn card_452_sky_hatch_runaway() {
    let (mut state, mut pool, mut rng) = setup_tavern(452);
    state
        .board
        .push(tier4::sky_hatch_runaway::template().instantiate());
    state
        .board
        .push(tier1::glim_guardian::template().instantiate()); // 1/3 Rally: Gain +2 Attack
    state
        .step(
            TavernAction::Activate {
                board_pos: 0,
                target_pos: Some(1),
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.board[1].attack, 3);
}

#[test]
fn card_453_snare_trapper() {
    let (mut state, mut pool, mut rng) = setup_tavern(453);
    state.add_to_hand(tier4::snare_trapper::template().instantiate());
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
    // Choose option 1: +1 Max Gold
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 1 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.auras.base_max_gold_bonus, 1);
}

#[test]
fn card_454_snarky_shark() {
    let (mut state, mut pool, mut rng) = setup_tavern(454);
    state
        .board
        .push(Unit::new("BeastAlly", 5, 5).with_tribe(Tribe::Beast));
    state
        .board
        .push(tier4::snarky_shark::template().instantiate());
    state
        .step(TavernAction::Sell { board_pos: 1 }, &mut pool, &mut rng)
        .unwrap();
    // BeastAlly attacks the 0/2 Fishbait, kills it, and gains +5/+5!
    assert_eq!(state.board[0].attack, 10);
    assert_eq!(state.board[0].health, 10);
}

#[test]
fn card_455_soulkeeping_jailer() {
    let (mut state, mut pool, mut rng) = setup_tavern(455);
    state
        .board
        .push(tier4::soulkeeping_jailer::template().instantiate()); // 3/5 Demon
    state.shop.push(Unit::new("ShopMinion", 4, 6));
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
    assert!(state.shop.is_empty());
    assert_eq!(state.board[0].attack, 7);
    assert_eq!(state.board[0].health, 11);
}

#[test]
fn card_456_tavern_tempest() {
    let (mut state, mut pool, mut rng) = setup_tavern(456);
    state.add_to_hand(tier4::tavern_tempest::template().instantiate());
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
    assert!(state.hand[0].tribe.matches(Tribe::Elemental));
}

#[test]
fn card_457_tortollan_blue_shell() {
    let (mut state, mut pool, mut rng) = setup_tavern(457);
    state.gold = 2;
    state.last_combat_lost = true;
    state
        .board
        .push(tier4::tortollan_blue_shell::template().instantiate());
    state
        .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
        .unwrap();
    // Sells for 5 Gold instead of 1 -> 2 + 5 = 7 Gold!
    assert_eq!(state.gold, 7);
}

#[test]
fn card_458_twilight_tidehunter() {
    let (mut state, mut pool, mut rng) = setup_tavern(458);
    state
        .board
        .push(tier4::twilight_tidehunter::template().instantiate());
    state.add_to_hand(Unit::new("HandMinion", 2, 2));
    state.add_to_hand(spells::make_tavern_dish_banana());
    state
        .step(
            TavernAction::Play {
                hand_index: 1,
                board_pos: 0,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    // Left-most minion in hand gains +8/+8 (to 10/10)!
    assert_eq!(state.hand[0].attack, 10);
    assert_eq!(state.hand[0].health, 10);
}
