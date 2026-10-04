//! Per-card functional tests for all 12 Solo Tier 7 minions (Patch 36.6.3)
//! and verification that Tier 7 cannot be reached via normal Tavern upgrades.

use seaglass::cards::{minions, spells, tokens};
use seaglass::{
    base_copies_for_tier, base_upgrade_cost, full_catalog, simulate, solo_tier_7_catalog,
    BattleOutcome, CardPool, GameState, Keyword, PlayerAuras, Rng, TavernAction, TavernState,
    Tribe, Unit,
};

fn setup_tavern(seed: u64) -> (TavernState, CardPool, Rng) {
    let mut state = TavernState::new().with_shop_spells(true);
    state.tavern_tier = 6;
    state.gold = 10;
    state.max_gold = 10;
    let pool = CardPool::new(full_catalog());
    let rng = Rng::new(seed);
    (state, pool, rng)
}

#[test]
fn tier7_catalog_and_cannot_upgrade_to_tier7() {
    let cards = solo_tier_7_catalog();
    assert_eq!(cards.len(), 12);
    assert_eq!(full_catalog().len(), 252);
    assert_eq!(base_copies_for_tier(7), 5);
    assert_eq!(base_upgrade_cost(6), 0);

    let (mut state, mut pool, mut rng) = setup_tavern(700);
    // Every Tier 7 minion starts with 5 copies in the shared pool.
    for card in &cards {
        assert_eq!(card.tavern_tier, 7);
        assert!(card.card_id >= 701 && card.card_id <= 712);
        assert_eq!(pool.remaining_copies(card.card_id), 5);
    }

    // Cannot upgrade from Tier 6 to Tier 7!
    state.gold = 20;
    let err = state
        .step(TavernAction::UpgradeTavern, &mut pool, &mut rng)
        .unwrap_err();
    assert!(!err.is_empty());
    assert_eq!(state.tavern_tier, 6);

    // Normal shop refreshes at max Tier 6 never offer Tier 7 minions or Tier 7 spells.
    for _ in 0..10 {
        state
            .step(TavernAction::Refresh, &mut pool, &mut rng)
            .unwrap();
        assert!(state.shop.iter().all(|u| u.tavern_tier <= 6));
    }
}

#[test]
fn card_701_captain_sanders() {
    let (mut state, mut pool, mut rng) = setup_tavern(701);
    state
        .board
        .push(minions::cord_puller::template().instantiate()); // 1/1 plain Mech
    state.add_to_hand(minions::captain_sanders::template().instantiate());
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
    // Captain Sanders Battlecry makes a friendly Tier 6 or below minion Golden!
    assert!(state.board[0].is_golden);
    assert_eq!(state.board[0].attack, 2);
    assert_eq!(state.board[0].health, 2);
}

#[test]
fn card_702_champion_of_sargeras() {
    let (mut state, mut pool, mut rng) = setup_tavern(702);
    state.add_to_hand(minions::champion_of_sargeras::template().instantiate());
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
    // Battlecry gives minions in the Tavern +8/+8 this game!
    assert_eq!(state.auras.tavern_all_atk, 8);
    assert_eq!(state.auras.tavern_all_hp, 8);

    // Deathrattle in combat also gives minions in the Tavern +8/+8 this game!
    let res = state.resolve_combat_against(
        &[Unit::new("GiantEnemy", 30, 30)],
        1,
        &PlayerAuras::default(),
        &[],
        702,
    );
    assert_eq!(res.outcome, BattleOutcome::BWin);
    assert_eq!(state.auras.tavern_all_atk, 16);
    assert_eq!(state.auras.tavern_all_hp, 16);
}

#[test]
fn card_703_futurefin() {
    let (mut state, mut pool, mut rng) = setup_tavern(703);
    state.board.push(minions::futurefin::template().instantiate()); // 7/13 Murloc
    state.add_to_hand(Unit::new("LeftHandMinion", 2, 3));
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    // End of Turn: gives Futurefin's stats (7/13) to the left-most minion in hand (2/3 -> 9/16)!
    assert_eq!(state.hand[0].attack, 9);
    assert_eq!(state.hand[0].health, 16);
}

#[test]
fn card_704_highkeeper_ra() {
    let (mut state, mut pool, mut rng) = setup_tavern(704);
    state.add_to_hand(minions::highkeeper_ra::template().instantiate());
    // Battlecry: gets a random Tier 6 minion!
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
    assert_eq!(state.hand[0].tavern_tier, 6);

    // Rally & Deathrattle in combat: gets 2 more Tier 6 minions!
    state.hand.clear();
    let res = state.resolve_combat_against(
        &[Unit::new("GiantEnemy", 30, 30)],
        1,
        &PlayerAuras::default(),
        &[],
        704,
    );
    assert_eq!(res.outcome, BattleOutcome::BWin);
    assert_eq!(state.hand.len(), 2);
    assert!(state.hand.iter().all(|u| u.tavern_tier == 6));
}

#[test]
fn card_705_jailbird_juggernaut() {
    let juggernaut = minions::jailbird_juggernaut::template().instantiate(); // 6/15 Quilboar
    // Side A has 2 units (Juggernaut + 0/1 totem) so Side A attacks first!
    // Enemy is 10/6: when Juggernaut attacks, its Rally summons a 6/15 Blood Golem that attacks the 10/6 first and kills it!
    // Juggernaut's own strike is then aborted without Juggernaut taking any damage!
    let res = simulate(
        &[juggernaut, Unit::new("Totem", 0, 1)],
        &[Unit::new("Enemy", 10, 6)],
        &GameState::default(),
        705,
    );
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a.len(), 3);
    assert_eq!(res.survivors_a[0].health, 15);
    assert_eq!(res.survivors_a[1].card_id, tokens::TOKEN_BLOOD_GOLEM);
    assert_eq!(res.survivors_a[1].attack, 6);
    assert_eq!(res.survivors_a[1].health, 5);
}

#[test]
fn card_706_obsidian_ravager() {
    let ravager = minions::obsidian_ravager::template().instantiate(); // 7/7 Dragon
    let side_a = vec![
        ravager,
        Unit::new("Totem1", 0, 1),
        Unit::new("Totem2", 0, 1),
    ];
    let opp = vec![
        Unit::new("Left", 0, 7),
        Unit::new("MidTaunt", 10, 7).with_keyword(Keyword::Taunt),
    ];
    let res = simulate(&side_a, &opp, &GameState::default(), 706);
    // Side A (3 minions) attacks first: Ravager attacks MidTaunt (10/7), and its Rally deals 7 damage
    // to MidTaunt AND adjacent Left, killing both before combat damage!
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a[0].health, 7);
}

#[test]
fn card_707_polarizing_beatboxer() {
    let (mut state, mut pool, mut rng) = setup_tavern(707);
    state
        .board
        .push(minions::polarizing_beatboxer::template().instantiate()); // 5/10 Mech
    state
        .board
        .push(minions::cord_puller::template().instantiate()); // 1/1 Mech with Divine Shield
    state.add_to_hand(minions::lullabot::template().instantiate()); // 2/2 Magnetic Mech
    // Magnetize Lullabot onto Cord Puller at board_pos 1:
    // Polarizing Beatboxer at board_pos 0 ALSO gets a copy of Lullabot magnetized to it!
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
    assert_eq!(state.board[1].attack, 3);
    assert_eq!(state.board[1].health, 3);
    assert_eq!(state.board[0].attack, 7);
    assert_eq!(state.board[0].health, 12);
    assert_eq!(state.board[0].magnetizations_count, 1);
    assert_eq!(state.board[0].eot_health_bonus, 1);
}

#[test]
fn card_708_sha_of_fear() {
    let (mut state, mut pool, mut rng) = setup_tavern(708);
    state
        .board
        .push(minions::sha_of_fear::template().instantiate()); // 10/13 Aberration
    state.add_to_hand(spells::spell_by_name("Leaf Through the Pages").unwrap());
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
    // Casting a Tavern spell gives minions and Deity (base 1/1) +3/+3!
    assert_eq!(state.board[0].attack, 13);
    assert_eq!(state.board[0].health, 16);
    assert_eq!(state.auras.deity.attack, 4);
    assert_eq!(state.auras.deity.health, 4);
}

#[test]
fn card_709_stalwart_kodo() {
    let kodo = minions::stalwart_kodo::template().instantiate(); // 16/32 Beast
    let bonehead = minions::harmless_bonehead::template().instantiate(); // Deathrattle: summon two 1/1 Skeletons
    let rider = minions::risen_rider::template().instantiate(); // 2/1 Taunt Reborn (Reborn is 3rd summon)
    let opp = vec![Unit::new("Enemy1", 3, 2), Unit::new("Enemy2", 3, 2)];
    let res = simulate(&[bonehead, rider, kodo], &opp, &GameState::default(), 709);
    // Summoned Skeletons (1/1) gain Stalwart Kodo's maximum stats (+16/+32 -> 17/33)!
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert!(res
        .survivors_a
        .iter()
        .any(|u| u.card_id == tokens::TOKEN_SKELETON && u.attack == 17 && u.max_health == 33));
}

#[test]
fn card_710_stitched_salvager() {
    let mut neighbor = Unit::new("BigNeighbor", 25, 30).with_keyword(Keyword::DivineShield);
    neighbor.is_golden = true;
    let salvager = minions::stitched_salvager::template().instantiate(); // 16/4 Undead
    let opp = vec![Unit::new("Killer", 10, 16), Unit::new("Small", 1, 1)];
    let res = simulate(&[neighbor, salvager], &opp, &GameState::default(), 710);
    // Start of Combat: Stitched Salvager destroys BigNeighbor (25/30 Golden DS) and stores it.
    // Salvager (16/4) trades with Killer (10/16) and dies -> Deathrattle resummons exact 25/30 Golden DS BigNeighbor!
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a.len(), 1);
    assert_eq!(res.survivors_a[0].name, "BigNeighbor");
    assert_eq!(res.survivors_a[0].attack, 25);
    assert_eq!(res.survivors_a[0].max_health, 30);
    assert!(res.survivors_a[0].is_golden);
}

#[test]
fn card_711_stone_age_slab() {
    let (mut state, mut pool, mut rng) = setup_tavern(711);
    state
        .board
        .push(minions::stone_age_slab::template().instantiate()); // 10/10 Elemental
    state.start_turn(&mut pool, &mut rng);
    assert_eq!(state.board[0].charges, 1);

    state.shop.clear();
    state
        .shop
        .push(minions::cord_puller::template().instantiate()); // 1/1 Mech
    state
        .step(TavernAction::Buy { shop_index: 0 }, &mut pool, &mut rng)
        .unwrap();
    // Buying Cord Puller (1/1) gave it +20/+20 (21/21) and doubled its stats -> 42/42!
    assert_eq!(state.board[0].charges, 0);
    assert_eq!(state.hand[0].attack, 42);
    assert_eq!(state.hand[0].health, 42);
}

#[test]
fn card_712_the_last_one_standing() {
    let (mut state, _pool, _rng) = setup_tavern(712);
    state
        .board
        .push(minions::the_last_one_standing::template().instantiate()); // 15/15 All
    state
        .board
        .push(Unit::new("MyBeast", 2, 2).with_tribe(Tribe::Beast));
    let opp = vec![Unit::new("Dummy", 1, 1)];
    let res = state.resolve_combat_against(&opp, 1, &PlayerAuras::default(), &[], 712);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Rally gave a friendly minion of each type +15/+15 PERMANENTLY!
    // Both The Last One Standing (15/15 -> 30/30) and MyBeast (2/2 -> 17/17) kept +15/+15 on the Tavern board!
    assert_eq!(state.board[0].attack, 30);
    assert_eq!(state.board[0].health, 30);
    assert_eq!(state.board[1].attack, 17);
    assert_eq!(state.board[1].health, 17);
}
