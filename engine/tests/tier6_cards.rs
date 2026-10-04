use seaglass::cards::{spells, tier1, tier2, tier6, tokens};
use seaglass::{
    full_catalog, simulate, solo_tier_6_catalog, BattleOutcome, CardPool, GameState, Keyword, Rng,
    TavernAction, TavernState, Tribe, Unit,
};

fn setup_tavern(seed: u64) -> (TavernState, CardPool, Rng) {
    let mut state = TavernState::new();
    state.tavern_tier = 6;
    state.gold = 10;
    state.max_gold = 10;
    let pool = CardPool::new(full_catalog());
    let rng = Rng::new(seed);
    (state, pool, rng)
}

#[test]
fn tier6_catalog_has_32_cards() {
    assert_eq!(solo_tier_6_catalog().len(), 32);
    assert_eq!(full_catalog().len(), 240);
}

#[test]
fn card_601_auto_reveille() {
    let (mut state, mut pool, mut rng) = setup_tavern(601);
    state
        .board
        .push(tier6::auto_reveille::template().instantiate()); // 4/8 Mech
    state.shop.push(tier1::cord_puller::template().instantiate());
    state.shop.push(tier1::lullabot::template().instantiate());
    state
        .shop
        .push(tier1::bubble_gunner::template().instantiate());
    // Buy 3 cards -> Magnetizes a random Volumizer to Auto Reveille and gives a copy in hand!
    for _ in 0..3 {
        state
            .step(TavernAction::Buy { shop_index: 0 }, &mut pool, &mut rng)
            .unwrap();
    }
    assert_eq!(state.board[0].magnetizations_count, 1);
    assert!(state.board[0].attack > 4 || state.board[0].health > 8);
    assert_eq!(state.hand.len(), 4);
    assert!(matches!(
        state.hand[3].card_id,
        tier2::blue_volumizer::ID | tier2::green_volumizer::ID | tier2::red_volumizer::ID
    ));
}

#[test]
fn card_602_balinda_stonehearth() {
    let (mut state, mut pool, mut rng) = setup_tavern(602);
    state
        .board
        .push(tier6::balinda_stonehearth::template().instantiate());
    state.board.push(Unit::new("Target", 2, 2));
    state.add_to_hand(spells::spell_by_name("Fortify").unwrap()); // +0/+3 and Taunt
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
    // Casts twice: +0/+6 -> 2/8 with Taunt!
    assert_eq!(state.board[1].attack, 2);
    assert_eq!(state.board[1].health, 8);
    assert!(state.board[1].taunt);
}

#[test]
fn card_603_choral_mrrrglr() {
    let choral = tier6::choral_mrrrglr::template().instantiate(); // 6/6
    let gs = GameState {
        hand_a: vec![Unit::new("H1", 10, 12), Unit::new("H2", 5, 8)],
        ..Default::default()
    };
    let res = simulate(&[choral], &[Unit::new("Enemy", 1, 1)], &gs, 603);
    // Gains +15/+20 from hand -> 21/26 (21/25 after taking 1 damage)
    assert_eq!(res.survivors_a[0].attack, 21);
    assert_eq!(res.survivors_a[0].max_health, 26);
}

#[test]
fn card_604_crimson_vindicator() {
    let vindicator = tier6::crimson_vindicator::template().instantiate(); // 8/9 Dragon, Divine Shield
    let ally = Unit::new("DragonAlly", 2, 2)
        .with_tribe(Tribe::Dragon)
        .with_keyword(Keyword::DivineShield);
    let enemy = Unit::new("Dummy", 1, 1);
    let res = simulate(&[vindicator, ally], &[enemy], &GameState::default(), 604);
    // Vindicator attacks -> Rally casts Mighty Dragonbreath (+3/+2 base +3/+2 Dragon +3/+2 DS = +9/+6) -> DragonAlly is 11/8!
    let surv_ally = res
        .survivors_a
        .iter()
        .find(|u| u.name == "DragonAlly")
        .unwrap();
    assert_eq!(surv_ally.attack, 11);
    assert_eq!(surv_ally.health, 8);
}

#[test]
fn card_605_dark_puppeteer() {
    let puppeteer = tier6::dark_puppeteer::template().instantiate(); // 8/4 Aberration
    let enemy = Unit::new("Killer", 10, 10);
    let res = simulate(&[puppeteer], &[enemy], &GameState::default(), 605);
    // Dark Puppeteer dies -> your Tavern spells give an extra +4 Health this game!
    assert_eq!(res.auras_a.spell_bonus_hp, 4);
}

#[test]
fn card_606_deathly_striker() {
    // 1) Test Avenge (4): 4 friendly deaths -> adds a random Undead to hand_a
    let f1 = Unit::new("F1", 1, 1).with_keyword(Keyword::Taunt);
    let f2 = Unit::new("F2", 1, 1).with_keyword(Keyword::Taunt);
    let f3 = Unit::new("F3", 1, 1).with_keyword(Keyword::Taunt);
    let f4 = Unit::new("F4", 1, 1).with_keyword(Keyword::Taunt);
    let striker = tier6::deathly_striker::template().instantiate();
    let enemy = Unit::new("Cleaver", 2, 20);
    let res = simulate(
        &[f1, f2, f3, f4, striker],
        &[enemy],
        &GameState::default(),
        606,
    );
    assert!(!res.hand_a.is_empty());
    assert!(res.hand_a[0].tribe.matches(Tribe::Undead));

    // 2) Test Deathrattle: summons an Undead from hand_a for this combat only
    let striker2 = tier6::deathly_striker::template().instantiate();
    let hand_undead = Unit::new("HandUndead", 20, 20).with_tribe(Tribe::Undead);
    let gs = GameState {
        hand_a: vec![hand_undead],
        ..Default::default()
    };
    let res2 = simulate(&[striker2], &[Unit::new("Killer", 15, 10)], &gs, 606);
    assert_eq!(res2.outcome, BattleOutcome::AWin);
    assert!(res2.survivors_a.iter().any(|u| u.name == "HandUndead"));
}

#[test]
fn card_607_deathstrider() {
    // Pair a Rally minion (Heroic Broodmother) with a Deathrattle minion (Harmless Bonehead) and Deathstrider
    let rally_unit = tier6::heroic_broodmother::template().instantiate();
    let bonehead = tier1::harmless_bonehead::template().instantiate();
    let strider = tier6::deathstrider::template().instantiate();
    let enemy = Unit::new("Dummy", 1, 7);
    let res = simulate(
        &[rally_unit, bonehead, strider],
        &[enemy],
        &GameState::default(),
        607,
    );
    // When Heroic Broodmother attacks, Deathstrider triggers left-most Deathrattle (Harmless Bonehead) -> summons Skeletons!
    assert!(res
        .survivors_a
        .iter()
        .any(|u| u.card_id == tokens::TOKEN_SKELETON));
}

#[test]
fn card_608_elemental_of_surprise() {
    let (mut state, mut pool, mut rng) = setup_tavern(608);
    // Put two Dune Dwellers on board, then buy Elemental of Surprise -> triples into Golden Dune Dweller with Divine Shield!
    state
        .board
        .push(tier1::dune_dweller::template().instantiate());
    state
        .board
        .push(tier1::dune_dweller::template().instantiate());
    state
        .shop
        .push(tier6::elemental_of_surprise::template().instantiate());
    state
        .step(TavernAction::Buy { shop_index: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert!(state.board.is_empty());
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tier1::dune_dweller::ID);
    assert!(state.hand[0].is_golden);
    assert!(state.hand[0].divine_shield);
}

#[test]
fn card_609_eredar_escapist() {
    let (mut state, mut pool, mut rng) = setup_tavern(609);
    state
        .board
        .push(tier6::eredar_escapist::template().instantiate()); // 6/8 Demon
    // Take 4 hero damage -> gets a copy of Corrupted Cupcakes in hand!
    state.deal_hero_damage(4);
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, spells::SPELL_CORRUPTED_CUPCAKES);
    let _ = (&mut pool, &mut rng);
}

#[test]
fn card_610_falling_sky_golem() {
    let (mut state, mut pool, mut rng) = setup_tavern(610);
    state.auras.deathrattles_triggered = 3;
    let mut golem = tier6::falling_sky_golem::template().instantiate(); // 4/2 Divine Shield
    state.apply_global_unit_auras(&mut golem);
    // +4/+2 per Deathrattle triggered (3 * +4/+2 = +12/+6) -> 16/8!
    assert_eq!(golem.attack, 16);
    assert_eq!(golem.health, 8);
    let _ = (&mut pool, &mut rng);
}

#[test]
fn card_611_forsaken_weaver() {
    let (mut state, mut pool, mut rng) = setup_tavern(611);
    state
        .board
        .push(tier6::forsaken_weaver::template().instantiate()); // 3/8 Undead
    state.add_to_hand(spells::spell_by_name("Tavern Coin").unwrap());
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
    // After casting a Tavern spell, Undead gain +3 Attack this game -> Weaver is 6/8!
    assert_eq!(state.auras.undead_bonus_attack, 3);
    assert_eq!(state.board[0].attack, 6);
}

#[test]
fn card_612_gatekeeper_amalgam() {
    let (mut state, mut pool, mut rng) = setup_tavern(612);
    state
        .board
        .push(tier6::gatekeeper_amalgam::template().instantiate()); // 6/6 All
    state
        .board
        .push(Unit::new("BeastAlly", 2, 2).with_tribe(Tribe::Beast));
    state.add_to_hand(spells::spell_by_name("Fortify").unwrap()); // +3/+3 Taunt
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
    // Casting a spell on Gatekeeper Amalgam casts Misplaced Tea Set (+4/+4 to a friendly minion of each type -> BeastAlly becomes 6/6)!
    assert_eq!(state.board[1].attack, 6);
    assert_eq!(state.board[1].health, 6);
}

#[test]
fn card_613_harbinger_aphlass() {
    let (mut state, mut pool, mut rng) = setup_tavern(613);
    state
        .board
        .push(tier6::harbinger_aphlass::template().instantiate());
    state.board.push(tier2::brain_rotter::template().instantiate());
    state.add_to_hand(tokens::make_blood_gem());
    state.add_to_hand(tokens::make_blood_gem());
    // Discard 1st card via Brain Rotter -> Aph'lass gives Deity +2/+1 (+ Brain Rotter +2/+2 = +4/+3 -> 5/4)
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
    assert_eq!(state.auras.deity.attack, 5);
    assert_eq!(state.auras.deity.health, 4);

    // Reset Brain Rotter activation and discard 2nd card -> Aph'lass improved to +4/+2 (+ Brain Rotter +2/+2 = +6/+4 -> 11/8)!
    state.board[1].activated_this_turn = false;
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
    assert_eq!(state.auras.deity.attack, 11);
    assert_eq!(state.auras.deity.health, 8);
}

#[test]
fn card_614_heroic_broodmother() {
    let broodmother = tier6::heroic_broodmother::template().instantiate(); // 7/7 Dragon
    let enemy = Unit::new("Enemy", 3, 5);
    let res = simulate(&[broodmother], &[enemy], &GameState::default(), 614);
    // Broodmother attacks immediately at Start of Combat; Rally grants Divine Shield before combat damage, so it takes 0 damage and kills the 3/5!
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a[0].health, 7);
}

#[test]
fn card_615_hooktusk_master_marauder() {
    let (mut state, mut pool, mut rng) = setup_tavern(615);
    state.auras.golden_minions_played = 2; // +1/+1 improved by 2 -> +3/+3
    state
        .board
        .push(tier6::hooktusk_master_marauder::template().instantiate());
    state
        .board
        .push(Unit::new("PirateAlly", 2, 2).with_tribe(Tribe::Pirate));
    state.add_to_hand(spells::spell_by_name("A New Sprout").unwrap());
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
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    // Discovering a card gives other Pirates +3/+3 -> PirateAlly is 5/5!
    assert_eq!(state.board[1].attack, 5);
    assert_eq!(state.board[1].health, 5);
}

#[test]
fn card_616_magicfin_mycologist() {
    let (mut state, mut pool, mut rng) = setup_tavern(616);
    state.add_to_hand(tier6::magicfin_mycologist::template().instantiate());
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
    assert_eq!(state.board[0].mycologist_charges_left, 1);

    // Put Azerite Empowerment (+2/+2 twice to all) in shop and buy it -> Mycologist gives a 1/1 Magicfin Apprentice taught that spell!
    state
        .shop
        .push(spells::spell_by_name("Azerite Empowerment").unwrap());
    state
        .step(TavernAction::Buy { shop_index: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.hand.len(), 2);
    let app_idx = state
        .hand
        .iter()
        .position(|u| u.card_id == tokens::TOKEN_MAGICFIN_APPRENTICE)
        .unwrap();
    assert_eq!(
        state.hand[app_idx].taught_spell_id,
        Some(spells::SPELL_AZERITE_EMPOWERMENT)
    );

    // Play the Apprentice -> Battlecry casts Azerite Empowerment (+4/+4 to all)!
    state
        .step(
            TavernAction::Play {
                hand_index: app_idx,
                board_pos: 1,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    // Mycologist (4/8) + 4/4 = 8/12
    assert_eq!(state.board[0].attack, 8);
    assert_eq!(state.board[0].health, 12);
}

#[test]
fn card_617_nadina_the_red() {
    let nadina = tier6::nadina_the_red::template().instantiate();
    let d1 = Unit::new("D1", 4, 4).with_tribe(Tribe::Dragon);
    let d2 = Unit::new("D2", 4, 4).with_tribe(Tribe::Dragon);
    let enemy = Unit::new("Killer", 10, 4);
    let res = simulate(&[nadina, d1, d2], &[enemy], &GameState::default(), 617);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert!(res.survivors_a.iter().all(|u| u.divine_shield || u.health == 4));
}

#[test]
fn card_618_ravaging_scorpid() {
    let scorpid = tier6::ravaging_scorpid::template().instantiate(); // 6/7 Beast
    let enemy = Unit::new("Killer", 10, 6);
    let res = simulate(&[scorpid], &[enemy], &GameState::default(), 618);
    // Scorpid attacks (+4/+4 Beetle bonus) and dies -> summons a 2/2 + 4/4 = 6/6 Beetle!
    assert_eq!(res.auras_a.beetle_bonus_atk, 4);
    assert_eq!(res.auras_a.beetle_bonus_hp, 4);
    assert_eq!(res.survivors_a.len(), 1);
    assert_eq!(res.survivors_a[0].card_id, tokens::TOKEN_BEETLE);
    assert_eq!(res.survivors_a[0].attack, 6);
    assert_eq!(res.survivors_a[0].health, 6);
}

#[test]
fn card_619_sanguine_champion() {
    let (mut state, mut pool, mut rng) = setup_tavern(619);
    state.add_to_hand(tier6::sanguine_champion::template().instantiate());
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
    // Battlecry: +2/+1 to Blood Gems
    assert_eq!(state.auras.blood_gem_bonus_atk, 2);
    assert_eq!(state.auras.blood_gem_bonus_hp, 1);

    // Also test Deathrattle in combat (+2/+1 more -> +4/+2)
    let gs = GameState {
        auras_a: state.auras.clone(),
        ..Default::default()
    };
    let res = simulate(&state.board, &[Unit::new("Killer", 20, 20)], &gs, 619);
    assert_eq!(res.auras_a.blood_gem_bonus_atk, 4);
    assert_eq!(res.auras_a.blood_gem_bonus_hp, 2);
}

#[test]
fn card_620_silent_deliverer() {
    let (mut state, mut pool, mut rng) = setup_tavern(620);
    state.add_to_hand(tier6::silent_deliverer::template().instantiate());
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
    // Battlecry: Get a random Golden minion from Tier 4
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].tavern_tier, 4);
    assert!(state.hand[0].is_golden);
}

#[test]
fn card_621_sky_admiral_rogers() {
    let (mut state, mut pool, mut rng) = setup_tavern(621);
    state
        .board
        .push(tier6::sky_admiral_rogers::template().instantiate()); // 4/5 Pirate
    // Spend 9 Gold via 9 Refreshes -> gets a random Bounty spell in hand!
    for _ in 0..9 {
        state
            .step(TavernAction::Refresh, &mut pool, &mut rng)
            .unwrap();
    }
    assert_eq!(state.hand.len(), 1);
    assert!(state.hand[0].is_spell);
}

#[test]
fn card_622_snazzy_phantom() {
    let rider = tier1::risen_rider::template().instantiate(); // 2/1 Taunt Reborn Undead
    let phantom = tier6::snazzy_phantom::template().instantiate(); // 6/8 Undead (right-most)
    let enemy = Unit::new("Enemy", 2, 4);
    let res = simulate(&[rider, phantom], &[enemy], &GameState::default(), 622);
    // Risen Rider (2 Attack) is Reborn -> Snazzy Phantom gives +2/+2 to right-most Undead (itself -> 8/10)!
    let surv_phantom = res
        .survivors_a
        .iter()
        .find(|u| u.card_id == tier6::snazzy_phantom::ID)
        .unwrap();
    assert_eq!(surv_phantom.attack, 8);
    assert_eq!(surv_phantom.max_health, 10);
}

#[test]
fn card_623_the_shadow_of_doubt() {
    let (mut state, mut pool, mut rng) = setup_tavern(623);
    state
        .board
        .push(tier6::the_shadow_of_doubt::template().instantiate());
    let pre_atk = state.auras.deity.attack;
    let pre_hp = state.auras.deity.health;
    state.add_to_hand(tokens::make_blood_gem());
    // Adding a card to hand gives Deity +4/+5!
    assert_eq!(state.auras.deity.attack, pre_atk + 4);
    assert_eq!(state.auras.deity.health, pre_hp + 5);
    let _ = (&mut pool, &mut rng);
}

#[test]
fn card_624_turbo_hogrider() {
    let (mut state, mut pool, mut rng) = setup_tavern(624);
    state
        .board
        .push(tier6::turbo_hogrider::template().instantiate()); // 6/8 Quilboar
    state.add_to_hand(spells::spell_by_name("Alliance Flag").unwrap());
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
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    // Alliance Flag option 0 (+3/+1) + Turbo Hogrider plays 2 Blood Gems (+2/+2) -> 11/11!
    assert_eq!(state.board[0].attack, 11);
    assert_eq!(state.board[0].health, 11);
}

#[test]
fn card_625_twisted_wrathguard() {
    let (mut state, mut pool, mut rng) = setup_tavern(625);
    state
        .board
        .push(tier6::twisted_wrathguard::template().instantiate()); // 8/8 Demon
    state.board.push(Unit::new("SellMe", 1, 1));
    state
        .step(TavernAction::Sell { board_pos: 1 }, &mut pool, &mut rng)
        .unwrap();
    // Selling a minion adds 1 Fodder to next Refresh!
    assert_eq!(state.auras.fodder_per_refresh[0], 1);
}

#[test]
fn card_626_tyrael() {
    let (mut state, mut pool, mut rng) = setup_tavern(626);
    state.board.push(tier6::tyrael::template().instantiate()); // 10/10 Activate(1)
    state.board.push(Unit::new("Small", 1, 1));
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
    // Sets target minion's stats to 50/50!
    assert_eq!(state.board[1].attack, 50);
    assert_eq!(state.board[1].health, 50);
}

#[test]
fn card_627_ultraviolet_ascendant() {
    let (mut state, mut pool, mut rng) = setup_tavern(627);
    state
        .board
        .push(tier6::ultraviolet_ascendant::template().instantiate()); // 6/6, starts at +3/+3
    state.add_to_hand(tier1::dune_dweller::template().instantiate()); // 3/2 Elemental
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
    // Playing Dune Dweller upgrades Ultraviolet Ascendant to 1 stack (+6/+6)!
    assert_eq!(state.board[0].ultraviolet_stacks, 1);

    let res = simulate(
        &state.board,
        &[Unit::new("Enemy", 1, 1)],
        &GameState::default(),
        627,
    );
    let surv_dune = res
        .survivors_a
        .iter()
        .find(|u| u.card_id == tier1::dune_dweller::ID)
        .unwrap();
    // 3/3 + 6/6 = 9/9!
    assert_eq!(surv_dune.attack, 9);
    assert_eq!(surv_dune.health, 9);
}

#[test]
fn card_628_unbound_tempest() {
    let (mut state, mut pool, mut rng) = setup_tavern(628);
    state
        .board
        .push(tier6::unbound_tempest::template().instantiate()); // 3/12 Elemental
    state.shop.push(Unit::new("BigShop", 10, 20));
    let elems = [
        tier1::dune_dweller::template().instantiate(),
        tier1::dune_dweller::template().instantiate(),
        tier1::crackling_cyclone::template().instantiate(),
        tier1::crackling_cyclone::template().instantiate(),
    ];
    for elem in elems {
        state.add_to_hand(elem);
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
    }
    // After 4 Elementals played, gains stats of highest-Health minion in Tavern (10/20 + 2/2 from 2 Dune Dwellers = 12/22, or 10/20 since BigShop is non-Elemental) -> +10/+20 -> 13/32!
    assert_eq!(state.board[0].attack, 13);
    assert_eq!(state.board[0].health, 32);
}

#[test]
fn card_629_utility_drone() {
    let (mut state, mut pool, mut rng) = setup_tavern(629);
    state
        .board
        .push(tier6::utility_drone::template().instantiate());
    let mut mech = Unit::new("MagMech", 3, 3).with_tribe(Tribe::Mech);
    mech.magnetizations_count = 2;
    state.board.push(mech);
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    // 2 Magnetizations * +4/+5 = +8/+10 -> 11/13!
    assert_eq!(state.board[1].attack, 11);
    assert_eq!(state.board[1].health, 13);
}

#[test]
fn card_630_veteran_brigand() {
    let (mut state, mut pool, mut rng) = setup_tavern(630);
    state.board.push(Unit::new("Ally", 2, 2));
    state.add_to_hand(tier6::veteran_brigand::template().instantiate()); // 8/8
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
    // Option 0: Play 3 Blood Gems on all your minions (+3/+3)
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 5);
    assert_eq!(state.board[1].attack, 11);
    assert_eq!(state.board[1].health, 11);
}

#[test]
fn card_631_victorious_geomant() {
    let (mut state, mut pool, mut rng) = setup_tavern(631);
    state
        .board
        .push(tier6::victorious_geomant::template().instantiate()); // 10/10 Activate(2)
    // Fill hand with 8 cards so only 2 of the 6 Blood Gems fit in hand, and 4 overflow onto left-most minion!
    for _ in 0..8 {
        state.add_to_hand(tokens::make_blood_gem());
    }
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
    assert_eq!(state.hand.len(), 10);
    assert_eq!(state.board[0].attack, 14);
    assert_eq!(state.board[0].health, 14);
}

#[test]
fn card_632_young_murk_eye() {
    let (mut state, mut pool, mut rng) = setup_tavern(632);
    state
        .board
        .push(tier1::razorfen_geomancer::template().instantiate()); // Battlecry: Get 2 Blood Gems
    state
        .board
        .push(tier6::young_murk_eye::template().instantiate());
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    // Young Murk-Eye triggers adjacent Razorfen Geomancer's Battlecry -> adds 2 Blood Gems to hand!
    assert_eq!(state.hand.len(), 2);
    assert!(state
        .hand
        .iter()
        .all(|u| u.card_id == tokens::SPELL_BLOOD_GEM));
}
