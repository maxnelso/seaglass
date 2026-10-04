use seaglass::cards::{spells, tier1, tier2, tier5, tokens};
use seaglass::{
    full_catalog, simulate, BattleOutcome, CardPool, GameState, Keyword, Rng, TavernAction,
    TavernState, Tribe, Unit,
};

fn setup_tavern(seed: u64) -> (TavernState, CardPool, Rng) {
    let mut state = TavernState::new();
    state.tavern_tier = 5;
    state.gold = 10;
    state.max_gold = 10;
    let pool = CardPool::new(full_catalog());
    let rng = Rng::new(seed);
    (state, pool, rng)
}

#[test]
fn card_501_air_revenant() {
    let (mut state, mut pool, mut rng) = setup_tavern(501);
    state
        .board
        .push(tier5::air_revenant::template().instantiate());
    // Spend 7 Gold via 7 Refreshes -> casts Easterly Winds (+9/+9 on each Refresh)!
    for _ in 0..7 {
        state
            .step(TavernAction::Refresh, &mut pool, &mut rng)
            .unwrap();
    }
    assert_eq!(state.auras.refresh_random_buffs, vec![(9, 9)]);
}

#[test]
fn card_502_barrier_banshee() {
    let banshee = tier5::barrier_banshee::template().instantiate(); // 8/8 Undead
    let rider = tier1::risen_rider::template().instantiate(); // 2/1 Taunt Reborn Undead
    let enemy = Unit::new("Enemy", 2, 4);
    let res = simulate(&[rider, banshee], &[enemy], &GameState::default(), 502);
    // Risen Rider dies, Reborn summons 2/1, Barrier Banshee gains Divine Shield and +8/+8 -> 16/16!
    let surv_banshee = res
        .survivors_a
        .iter()
        .find(|u| u.card_id == tier5::barrier_banshee::ID)
        .unwrap();
    assert_eq!(surv_banshee.attack, 16);
    assert_eq!(surv_banshee.health, 16);
    assert!(surv_banshee.divine_shield);
}

#[test]
fn card_503_bile_spitter() {
    let spitter = tier5::bile_spitter::template().instantiate(); // 3/10 Venomous Murloc
    let murloc = Unit::new("Buddy", 5, 5).with_tribe(Tribe::Murloc);
    let enemy = Unit::new("Tank", 1, 100);
    let res = simulate(&[spitter, murloc], &[enemy], &GameState::default(), 503);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    let buddy = res
        .survivors_a
        .iter()
        .find(|u| u.name == "Buddy")
        .unwrap();
    assert!(buddy.venomous || res.survivors_b.is_empty());
}

#[test]
fn card_504_brann_bronzebeard() {
    let (mut state, mut pool, mut rng) = setup_tavern(504);
    state
        .board
        .push(tier5::brann_bronzebeard::template().instantiate());
    state.add_to_hand(tier1::razorfen_geomancer::template().instantiate());
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
    // Razorfen Geomancer gives 2 Blood Gems; with Brann it triggers twice -> 4 Blood Gems!
    assert_eq!(state.hand.len(), 4);
    assert!(state.hand.iter().all(|c| c.card_id == tokens::SPELL_BLOOD_GEM));
}

#[test]
fn card_505_cataclysmic_harbinger() {
    let (mut state, mut pool, mut rng) = setup_tavern(505);
    state
        .board
        .push(tier5::cataclysmic_harbinger::template().instantiate()); // 6/10 Elemental
    state.add_to_hand(spells::spell_by_name("Armor Stash").unwrap());
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
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    // At the end of turn, gets a copy of the last Tavern spell cast (Armor Stash)!
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, spells::SPELL_ARMOR_STASH);
}

#[test]
fn card_506_charging_czarina() {
    let (mut state, mut pool, mut rng) = setup_tavern(506);
    state
        .board
        .push(tier5::charging_czarina::template().instantiate()); // 4/1 Divine Shield
    state
        .board
        .push(Unit::new("Plain", 2, 2));
    state.add_to_hand(spells::spell_by_name("Tavern Dish Banana").unwrap());
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
    // Czarina has Divine Shield so she gains +4 Attack (4 -> 8)!
    assert_eq!(state.board[0].attack, 8);
}

#[test]
fn card_507_costume_enthusiast() {
    let enthusiast = tier5::costume_enthusiast::template().instantiate(); // 4/5 DS
    let mut gs = GameState::default();
    gs.hand_a.push(Unit::new("BigHand", 12, 4));
    let res = simulate(
        &[enthusiast],
        &[Unit::new("Enemy", 1, 1)],
        &gs,
        507,
    );
    // Gains +12 Attack from BigHand -> 16 Attack!
    assert_eq!(res.survivors_a[0].attack, 16);
}

#[test]
fn card_508_de_volition_ist() {
    let devo = tier5::de_volition_ist::template().instantiate(); // 4/8
    let e1 = Unit::new("Taunt", 1, 5).with_keyword(Keyword::Taunt);
    let e2 = Unit::new("Backline", 1, 10);
    let res = simulate(&[devo], &[e1, e2], &GameState::default(), 508);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Verify De-volition-ist dealt 4 bonus damage after attacking!
    let bonus_hits = res
        .events
        .iter()
        .filter(|e| matches!(e, seaglass::Event::DamageDealt { amount: 4, .. }))
        .count();
    assert!(bonus_hits >= 4);
}

#[test]
fn card_509_deft_deserter() {
    let (mut state, mut pool, mut rng) = setup_tavern(509);
    state
        .board
        .push(tier5::deft_deserter::template().instantiate()); // 8/8 Demon
    state.shop.push(Unit::new("ShopMinion", 2, 2));
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
    // Gives all minions in the Tavern +8/+8 and Taunt, Divine Shield, or Windfury!
    assert_eq!(state.shop[0].attack, 10);
    assert_eq!(state.shop[0].health, 10);
    assert!(state.shop[0].taunt || state.shop[0].divine_shield || state.shop[0].windfury);
}

#[test]
fn card_510_devilish_distractor() {
    let (mut state, mut pool, mut rng) = setup_tavern(510);
    state
        .board
        .push(tier5::devilish_distractor::template().instantiate()); // 4/7 Demon
    state.shop.push(Unit::new("ShopMinion", 2, 2));
    state.add_to_hand(spells::spell_by_name("Tavern Dish Banana").unwrap());
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
    // Banana gives +2/+2 -> 6/9, and Devilish Distractor gives minions in the Tavern +1/+2 this game!
    assert_eq!(state.board[0].attack, 6);
    assert_eq!(state.board[0].health, 9);
    assert_eq!(state.auras.tavern_all_atk, 1);
    assert_eq!(state.auras.tavern_all_hp, 2);
    assert_eq!(state.shop[0].attack, 3);
    assert_eq!(state.shop[0].health, 4);
}

#[test]
fn card_511_draconic_warden() {
    let (mut state, mut pool, mut rng) = setup_tavern(511);
    state.add_to_hand(tier5::draconic_warden::template().instantiate());
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
    assert!(tokens::CHROMADRAKE_IDS.contains(&state.hand[0].card_id));
}

#[test]
fn card_512_drakkari_enchanter() {
    let (mut state, mut pool, mut rng) = setup_tavern(512);
    state
        .board
        .push(tier5::drakkari_enchanter::template().instantiate());
    state
        .board
        .push(tier1::lullabot::template().instantiate()); // 2/2, gains +1 HP at end of turn
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    // Lullabot triggered twice (+2 HP -> 2/4)!
    assert_eq!(state.board[1].health, 4);
}

#[test]
fn card_513_drustfallen_butcher() {
    let butcher = tier5::drustfallen_butcher::template().instantiate();
    let f1 = Unit::new("F1", 1, 1).with_keyword(Keyword::Taunt);
    let f2 = Unit::new("F2", 1, 1).with_keyword(Keyword::Taunt);
    let f3 = Unit::new("F3", 1, 1).with_keyword(Keyword::Taunt);
    let f4 = Unit::new("F4", 1, 1).with_keyword(Keyword::Taunt);
    let enemy = Unit::new("Enemy", 2, 20);
    let res = simulate(
        &[f1, f2, f3, f4, butcher],
        &[enemy],
        &GameState::default(),
        513,
    );
    // After 4 friendly deaths, Drustfallen Butcher adds a Butchering to hand_a!
    assert_eq!(res.hand_a.len(), 1);
    assert_eq!(res.hand_a[0].card_id, spells::SPELL_BUTCHERING);
}

#[test]
fn card_514_elite_navigator() {
    let (mut state, mut pool, mut rng) = setup_tavern(514);
    state
        .board
        .push(tier1::southsea_busker::template().instantiate()); // Tier 1 Pirate 3/1
    state.add_to_hand(tier5::elite_navigator::template().instantiate());
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
    assert!(state.board[0].is_golden);
    assert_eq!(state.board[0].attack, 6);
    assert_eq!(state.board[0].health, 2);
}

#[test]
fn card_515_enterprising_escapee() {
    let (mut state, mut pool, mut rng) = setup_tavern(515);
    state
        .board
        .push(tier5::enterprising_escapee::template().instantiate());
    for _ in 0..6 {
        state
            .step(TavernAction::Refresh, &mut pool, &mut rng)
            .unwrap();
    }
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tokens::SPELL_LOCKBOX);
}

#[test]
fn card_516_eternal_summoner() {
    let summoner = tier5::eternal_summoner::template().instantiate();
    let enemy = Unit::new("Enemy", 10, 5);
    let res = simulate(&[summoner], &[enemy], &GameState::default(), 516);
    // Eternal Summoner dies and summons Eternal Knight + Reborn Eternal Summoner!
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert!(res
        .survivors_a
        .iter()
        .any(|u| u.card_id == tier2::eternal_knight::ID));
}

#[test]
fn card_517_eternal_tycoon() {
    let tycoon = tier5::eternal_tycoon::template().instantiate();
    let f1 = Unit::new("F1", 1, 1).with_keyword(Keyword::Taunt);
    let f2 = Unit::new("F2", 1, 1).with_keyword(Keyword::Taunt);
    let f3 = Unit::new("F3", 1, 1).with_keyword(Keyword::Taunt);
    let f4 = Unit::new("F4", 1, 1).with_keyword(Keyword::Taunt);
    let f5 = Unit::new("F5", 1, 1).with_keyword(Keyword::Taunt);
    let enemy = Unit::new("Enemy", 2, 10);
    let res = simulate(
        &[f1, f2, f3, f4, f5, tycoon],
        &[enemy],
        &GameState::default(),
        517,
    );
    assert!(res.events.iter().any(|e| matches!(
        e,
        seaglass::Event::UnitSummoned {
            reason: "Eternal Tycoon",
            ..
        }
    )));
}

#[test]
fn card_518_faceless_converter() {
    let converter = tier5::faceless_converter::template().instantiate();
    let enemy = Unit::new("Enemy", 10, 10);
    let res = simulate(&[converter], &[enemy], &GameState::default(), 518);
    // Deathrattle: Give your Deity +2/+1 (from base 1/1 -> 3/2)!
    assert_eq!(res.auras_a.deity.attack, 3);
    assert_eq!(res.auras_a.deity.health, 2);
}

#[test]
fn card_519_felboar() {
    let (mut state, mut pool, mut rng) = setup_tavern(519);
    state.board.push(tier5::felboar::template().instantiate()); // 2/6
    state.shop.push(Unit::new("Food", 5, 6));
    state.add_to_hand(tokens::make_blood_gem());
    state.add_to_hand(tokens::make_blood_gem());
    state.add_to_hand(tokens::make_blood_gem());
    for _ in 0..3 {
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
    }
    // 3 Blood Gems (+3/+3 -> 5/9) + consumed Food (5/6) = 10/15!
    assert!(state.shop.is_empty());
    assert_eq!(state.board[0].attack, 10);
    assert_eq!(state.board[0].health, 15);
}

#[test]
fn card_520_felfire_conjurer() {
    let (mut state, mut pool, mut rng) = setup_tavern(520);
    state
        .board
        .push(tier5::felfire_conjurer::template().instantiate());
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.auras.spell_bonus_atk, 1);
    assert_eq!(state.auras.spell_bonus_hp, 1);
}

#[test]
fn card_521_firelands_fugitive() {
    let (mut state, mut pool, mut rng) = setup_tavern(521);
    state.add_to_hand(tier5::firelands_fugitive::template().instantiate());
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
    assert_eq!(state.hand[0].card_id, tokens::SPELL_CONFLAGRATION);
}

#[test]
fn card_522_firescale_hoarder() {
    let (mut state, mut pool, mut rng) = setup_tavern(522);
    state.add_to_hand(tier5::firescale_hoarder::template().instantiate());
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
    assert_eq!(state.hand[0].card_id, spells::SPELL_SHINY_RING);
}

#[test]
fn card_523_ghastcoiler() {
    let coiler = tier5::ghastcoiler::template().instantiate();
    let enemy = Unit::new("Enemy", 10, 5);
    let res = simulate(&[coiler], &[enemy], &GameState::default(), 523);
    let summoned = res
        .events
        .iter()
        .filter(|e| {
            matches!(
                e,
                seaglass::Event::UnitSummoned {
                    side: seaglass::Side::A,
                    reason: "Deathrattle",
                    ..
                }
            )
        })
        .count();
    assert!(summoned >= 2);
}

#[test]
fn card_524_goldrinn_the_great_wolf() {
    let goldrinn = tier5::goldrinn_the_great_wolf::template()
        .instantiate()
        .with_keyword(Keyword::Taunt);
    let beast = tier1::buzzing_vermin::template().instantiate(); // 1/1 Beast
    let enemy = Unit::new("Enemy", 10, 5);
    let res = simulate(&[goldrinn, beast], &[enemy], &GameState::default(), 524);
    assert_eq!(res.auras_a.goldrinn_bonus, 7);
    let surv_beast = res
        .survivors_a
        .iter()
        .find(|u| u.card_id == tier1::buzzing_vermin::ID)
        .unwrap();
    assert_eq!(surv_beast.attack, 8);
    assert_eq!(surv_beast.health, 8);
}

#[test]
fn card_525_hackerfin() {
    let (mut state, mut pool, mut rng) = setup_tavern(525);
    state.board.push(
        Unit::new("KeywordUnit", 2, 2)
            .with_keyword(Keyword::Taunt)
            .with_keyword(Keyword::DivineShield),
    );
    state.add_to_hand(tier5::hackerfin::template().instantiate());
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
    // Base +3/+2 improved by 2 Bonus Keywords (mult = 1 + 2 = 3 -> +9/+6) -> 11/8!
    assert_eq!(state.board[0].attack, 11);
    assert_eq!(state.board[0].health, 8);
}

#[test]
fn card_526_hopebringer() {
    let hope = tier5::hopebringer::template().instantiate(); // 4/3
    let shield_ally = Unit::new("Shield", 2, 2)
        .with_keyword(Keyword::Taunt)
        .with_keyword(Keyword::DivineShield);
    let enemy = Unit::new("Enemy", 1, 1);
    let res = simulate(&[shield_ally, hope], &[enemy], &GameState::default(), 526);
    // Start of Combat: gives +4/+3 -> Hope is 8/6, Shield is 6/5.
    // Enemy hits Shield's Divine Shield -> Hopebringer gains +1 hopebringer_stacks!
    let surv_hope = res
        .survivors_a
        .iter()
        .find(|u| u.card_id == tier5::hopebringer::ID)
        .unwrap();
    assert_eq!(surv_hope.attack, 8);
    assert_eq!(surv_hope.health, 6);
    assert_eq!(surv_hope.hopebringer_stacks, 1);
}

#[test]
fn card_527_insatiable_urzul() {
    let (mut state, mut pool, mut rng) = setup_tavern(527);
    state
        .board
        .push(tier5::insatiable_urzul::template().instantiate()); // 4/6
    state.shop.push(Unit::new("Food", 5, 7));
    state.add_to_hand(tier1::ominous_seer::template().instantiate()); // Demon
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
    assert!(state.shop.is_empty());
    assert_eq!(state.board[0].attack, 9);
    assert_eq!(state.board[0].health, 13);
}

#[test]
fn card_528_kalecgos_arcane_aspect() {
    let (mut state, mut pool, mut rng) = setup_tavern(528);
    state
        .board
        .push(tier5::kalecgos_arcane_aspect::template().instantiate()); // 4/12 Dragon
    state.add_to_hand(tier1::southsea_busker::template().instantiate()); // Battlecry minion
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
    assert_eq!(state.board[0].attack, 6);
    assert_eq!(state.board[0].health, 14);
}

#[test]
fn card_529_leeroy_the_reckless() {
    let leeroy = tier5::leeroy_the_reckless::template().instantiate(); // 6/2
    let boss = Unit::new("GiantBoss", 50, 500).with_keyword(Keyword::DivineShield);
    let res = simulate(&[leeroy], &[boss], &GameState::default(), 529);
    // Leeroy pops DS or gets killed by GiantBoss, then Leeroy's Deathrattle destroys GiantBoss even through Divine Shield!
    assert_eq!(res.outcome, BattleOutcome::Draw);
    assert!(res.survivors_b.is_empty());
}

#[test]
fn card_530_lichling_hoarder() {
    let hoarder = tier5::lichling_hoarder::template().instantiate();
    let f1 = tier1::risen_rider::template().instantiate();
    let f2 = tier1::harmless_bonehead::template().instantiate();
    let enemy = Unit::new("Enemy", 3, 20);
    let res = simulate(&[f1, f2, hoarder], &[enemy], &GameState::default(), 530);
    // Risen Rider (2 deaths) + Harmless Bonehead (1 death) = 3 deaths -> Avenge (3) adds a plain copy to hand!
    assert!(!res.hand_a.is_empty());
}

#[test]
fn card_531_living_azerite() {
    let (mut state, mut pool, mut rng) = setup_tavern(531);
    state
        .board
        .push(tier5::living_azerite::template().instantiate());
    state.add_to_hand(spells::spell_by_name("Armor Stash").unwrap());
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
    assert_eq!(state.auras.tavern_elemental_atk, 4);
    assert_eq!(state.auras.tavern_elemental_hp, 3);
}

#[test]
fn card_532_lurking_leviathan() {
    let (mut state, mut pool, mut rng) = setup_tavern(532);
    state
        .board
        .push(tier5::lurking_leviathan::template().instantiate());
    state.add_to_hand(tier1::flittering_bat::template().instantiate()); // 1/4 Beast
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
    // Leviathan gives summoned Beast +3 Attack (4/4) and upgrades leviathan_stacks to 1!
    assert_eq!(state.board[1].attack, 4);
    assert_eq!(state.board[1].health, 4);
    assert_eq!(state.board[0].leviathan_stacks, 1);
}

#[test]
fn card_533_mindbender_ghursha() {
    let (mut state, mut pool, mut rng) = setup_tavern(533);
    state
        .board
        .push(tier5::mindbender_ghursha::template().instantiate()); // 3/9 Aberration
    state
        .board
        .push(tier2::brain_rotter::template().instantiate()); // 3/4 Aberration
    state.add_to_hand(tokens::make_blood_gem());
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
    // Ghur'sha gives OTHER friendly minions +4/+4 -> Brain Rotter (3/4) becomes 7/8!
    assert_eq!(state.board[1].attack, 7);
    assert_eq!(state.board[1].health, 8);
}

#[test]
fn card_534_mysterious_kthir() {
    let (mut state, mut pool, mut rng) = setup_tavern(534);
    state
        .board
        .push(tier5::mysterious_kthir::template().instantiate()); // 7/7 Aberration
    state.add_to_hand(spells::spell_by_name("Armor Stash").unwrap());
    state.add_to_hand(spells::spell_by_name("Tavern Dish Banana").unwrap());
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    // Discarded 2 Tavern spells and gained +7/+7 for each (+14/+14 -> 21/21)!
    assert_eq!(state.auras.cards_discarded, 2);
    assert!(state.hand.is_empty());
    assert_eq!(state.board[0].attack, 21);
    assert_eq!(state.board[0].health, 21);
}

#[test]
fn card_535_nraqi_frostcaller() {
    let (mut state, mut pool, mut rng) = setup_tavern(535);
    state
        .board
        .push(tier5::nraqi_frostcaller::template().instantiate());
    state.add_to_hand(Unit::new("Trash", 1, 1));
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
    assert_eq!(state.auras.spell_bonus_atk, 1);
    assert_eq!(state.auras.spell_bonus_hp, 1);
}

#[test]
fn card_536_nraqi_sapper() {
    let (mut state, mut pool, mut rng) = setup_tavern(536);
    state.add_to_hand(tier5::nraqi_sapper::template().instantiate());
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
    assert_eq!(state.hand[0].card_id, spells::SPELL_ENERGIZING_CHAMBER);
}

#[test]
fn card_537_nightmare_par_tea_guest() {
    let (mut state, mut pool, mut rng) = setup_tavern(537);
    state.add_to_hand(tier5::nightmare_par_tea_guest::template().instantiate());
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
    assert_eq!(state.hand[0].card_id, spells::SPELL_MISPLACED_TEA_SET);
}

#[test]
fn card_538_primalfin_lookout() {
    let (mut state, mut pool, mut rng) = setup_tavern(538);
    state
        .board
        .push(tier1::bubble_gunner::template().instantiate()); // Friendly Murloc on board
    state.add_to_hand(tier5::primalfin_lookout::template().instantiate());
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
    assert!(state.discover_pending.is_some());
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.hand.len(), 1);
    assert!(state.hand[0].tribe.matches(Tribe::Murloc));
}

#[test]
fn card_539_proud_privateer() {
    let (mut state, mut pool, mut rng) = setup_tavern(539);
    state
        .board
        .push(tier5::proud_privateer::template().instantiate()); // 8/8 Pirate
    state.add_to_hand(spells::spell_by_name("Selfish Bounty").unwrap()); // +6/+6
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
    // Casts twice (+12/+12 -> 20/20)!
    assert_eq!(state.board[0].attack, 20);
    assert_eq!(state.board[0].health, 20);
}

#[test]
fn card_540_razorfen_vineweaver() {
    let (mut state, _pool, _rng) = setup_tavern(540);
    state
        .board
        .push(tier5::razorfen_vineweaver::template().instantiate()); // 4/4 Quilboar
    let res = state.resolve_combat_against(
        &[Unit::new("Enemy", 1, 1)],
        1,
        &Default::default(),
        &[],
        540,
    );
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Gained 4 Blood Gems permanently in combat (+4/+4 -> 8/8 in Tavern)!
    assert_eq!(state.board[0].attack, 8);
    assert_eq!(state.board[0].health, 8);
    assert_eq!(state.board[0].blood_gems_played, 4);
}

#[test]
fn card_541_resourceful_robot() {
    let (mut state, mut pool, mut rng) = setup_tavern(541);
    state
        .board
        .push(tier5::resourceful_robot::template().instantiate()); // 4/5
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    // Magnetized a Volumizer to itself!
    assert!(state.board[0].attack > 4 || state.board[0].health > 5);
}

#[test]
fn card_542_rodeo_performer() {
    let (mut state, mut pool, mut rng) = setup_tavern(542);
    state.add_to_hand(tier5::rodeo_performer::template().instantiate());
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
    assert!(state.hand[0].is_spell);
}

#[test]
fn card_543_sanguine_refiner() {
    let refiner = tier5::sanguine_refiner::template().instantiate();
    let enemy = Unit::new("Enemy", 1, 5);
    let res = simulate(&[refiner], &[enemy], &GameState::default(), 543);
    assert_eq!(res.auras_a.blood_gem_bonus_atk, 1);
    assert_eq!(res.auras_a.blood_gem_bonus_hp, 2);
}

#[test]
fn card_544_sewer_escapee() {
    let (mut state, mut pool, mut rng) = setup_tavern(544);
    state
        .board
        .push(tier5::sewer_escapee::template().instantiate());
    state
        .board
        .push(Unit::new("OtherMurloc", 2, 2).with_tribe(Tribe::Murloc));
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
    assert_eq!(state.board[1].attack, 9);
    assert_eq!(state.board[1].health, 9);
}

#[test]
fn card_545_sewer_lord() {
    let lord = tier5::sewer_lord::template().instantiate(); // 4/6
    let enemy = Unit::new("Cleaver", 6, 20);
    let res = simulate(&[lord], &[enemy], &GameState::default(), 545);
    // Sewer Lord summons two 3/2 Sewer Rats, each of which summons a 2/3 Half-Shell with Taunt!
    assert!(res.events.iter().any(|e| matches!(
        e,
        seaglass::Event::UnitSummoned {
            name,
            ..
        } if name == "Sewer Rat"
    )));
    assert!(res.events.iter().any(|e| matches!(
        e,
        seaglass::Event::UnitSummoned {
            name,
            ..
        } if name == "Half-Shell"
    )));
}

#[test]
fn card_546_shamanic_tidecaller() {
    let (mut state, mut pool, mut rng) = setup_tavern(546);
    state
        .board
        .push(tier5::shamanic_tidecaller::template().instantiate()); // 5/7 Murloc
    state.add_to_hand(spells::spell_by_name("Tavern Dish Banana").unwrap());
    state.add_to_hand(Unit::new("HandMurloc", 3, 3).with_tribe(Tribe::Murloc));
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
    // Cast spell on Murloc -> Murlocs in hand and board gain +3/+3!
    // Tidecaller: 5/7 + 2/2 (Banana) + 3/3 = 10/12; HandMurloc: 3/3 + 3/3 = 6/6!
    assert_eq!(state.board[0].attack, 10);
    assert_eq!(state.board[0].health, 12);
    assert_eq!(state.hand[0].attack, 6);
    assert_eq!(state.hand[0].health, 6);
}

#[test]
fn card_547_ship_master_eudora() {
    let (mut state, _pool, _rng) = setup_tavern(547);
    state
        .board
        .push(tier5::ship_master_eudora::template().instantiate().with_keyword(Keyword::Taunt));
    state
        .board
        .push(tier1::aureate_laureate::template().instantiate()); // Golden 2/2 DS
    let _res = state.resolve_combat_against(
        &[Unit::new("Enemy", 10, 5)],
        1,
        &Default::default(),
        &[],
        547,
    );
    // Golden Aureate Laureate permanently kept +6/+6 from Eudora -> 8/8!
    assert_eq!(state.board[1].attack, 8);
    assert_eq!(state.board[1].health, 8);
}

#[test]
fn card_548_shipwrecked_rascal() {
    let (mut state, mut pool, mut rng) = setup_tavern(548);
    state.add_to_hand(tier5::shipwrecked_rascal::template().instantiate());
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
    assert!(spells::BOUNTY_SPELL_IDS.contains(&state.hand[0].card_id));
}

#[test]
fn card_549_spark_snapper() {
    let (mut state, mut pool, mut rng) = setup_tavern(549);
    state
        .board
        .push(tier5::spark_snapper::template().instantiate());
    state.add_to_hand(tier1::cord_puller::template().instantiate()); // 1/1 Mech
    state.add_to_hand(tier1::lullabot::template().instantiate()); // 2/2 Mech
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
    // First Mech (1/1) gets +2/+3 Satellite -> 3/4!
    assert_eq!(state.board[1].attack, 3);
    assert_eq!(state.board[1].health, 4);
    // Play second Mech (not magnetized, at pos 2): gets upgraded +4/+6 Satellite -> 6/8!
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
    assert_eq!(state.board[2].attack, 6);
    assert_eq!(state.board[2].health, 8);
}

#[test]
fn card_550_tichondrius() {
    let (mut state, _pool, _rng) = setup_tavern(550);
    state
        .board
        .push(tier5::tichondrius::template().instantiate()); // 4/4 Demon
    state.deal_hero_damage(1);
    assert_eq!(state.board[0].attack, 8);
    assert_eq!(state.board[0].health, 8);
}

#[test]
fn card_551_titus_rivendare() {
    let titus = tier5::titus_rivendare::template().instantiate();
    let bonehead = tier1::harmless_bonehead::template()
        .instantiate()
        .with_keyword(Keyword::Taunt);
    let enemy = Unit::new("Enemy", 5, 2);
    let res = simulate(&[bonehead, titus], &[enemy], &GameState::default(), 551);
    // Harmless Bonehead summons 2 Skeletons * 2 (Titus) = 4 Skeletons!
    let skels = res
        .survivors_a
        .iter()
        .filter(|u| u.card_id == tokens::TOKEN_SKELETON)
        .count();
    assert_eq!(skels, 4);
}

#[test]
fn card_552_turquoise_skitterer() {
    let skitterer = tier5::turquoise_skitterer::template().instantiate();
    let enemy = Unit::new("Enemy", 10, 2);
    let res = simulate(&[skitterer], &[enemy], &GameState::default(), 552);
    assert_eq!(res.auras_a.beetle_bonus_atk, 5);
    assert_eq!(res.auras_a.beetle_bonus_hp, 5);
    // Summoned Beetle has 2+5 / 2+5 = 7/7!
    assert_eq!(res.survivors_a.len(), 1);
    assert_eq!(res.survivors_a[0].attack, 7);
    assert_eq!(res.survivors_a[0].health, 7);
}
