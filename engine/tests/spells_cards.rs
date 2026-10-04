//! Exhaustive per-spell functional unit tests for all 30 Tavern Spells across Tiers 1-3
//! plus all 6 generated token spells (Patch 36.6.3).

use seaglass::cards::{spells, tier1, tier2, tier3, tokens};
use seaglass::{
    full_catalog, BattleOutcome, CardPool, Keyword, Rng, TavernAction, TavernState, Tribe, Unit,
};

fn setup_tavern(seed: u64) -> (TavernState, CardPool, Rng) {
    let pool = CardPool::new(full_catalog());
    let rng = Rng::new(seed);
    let mut state = TavernState::new().with_shop_spells(true);
    state.tavern_tier = 3;
    state.gold = 8;
    (state, pool, rng)
}

// ============================================================================
// Tier 1 Tavern Spells (8)
// ============================================================================

#[test]
fn spell_801_a_new_sprout() {
    let (mut state, mut pool, mut rng) = setup_tavern(801);
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
    assert!(state.discover_pending.is_some());
    assert!(state
        .discover_pending
        .as_ref()
        .unwrap()
        .iter()
        .all(|u| u.tavern_tier == 1));
}

#[test]
fn spell_802_alliance_flag() {
    let (mut state, mut pool, mut rng) = setup_tavern(802);
    state.board.push(Unit::new("Target", 2, 2));
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
    // Option 0: +3/+1
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 3);
}

#[test]
fn spell_803_enchanted_lasso() {
    let (mut state, mut pool, mut rng) = setup_tavern(803);
    state.shop.push(Unit::new("ShopMinion", 4, 4));
    state.add_to_hand(spells::spell_by_name("Enchanted Lasso").unwrap());
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
    assert_eq!(state.hand[0].name, "ShopMinion");
    assert!(state.shop.is_empty());
}

#[test]
fn spell_804_fortify() {
    let (mut state, mut pool, mut rng) = setup_tavern(804);
    state.board.push(Unit::new("Target", 2, 2));
    state.add_to_hand(spells::spell_by_name("Fortify").unwrap());
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
    assert_eq!(state.board[0].health, 5);
    assert!(state.board[0].taunt);
}

#[test]
fn spell_805_recruit_a_trainee() {
    let (mut state, mut pool, mut rng) = setup_tavern(805);
    state.add_to_hand(spells::spell_by_name("Recruit a Trainee").unwrap());
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
    assert_eq!(state.hand[0].tavern_tier, 1);
}

#[test]
fn spell_951_tavern_coin() {
    let (mut state, mut pool, mut rng) = setup_tavern(951);
    state.gold = 5;
    state.add_to_hand(tokens::make_tavern_coin());
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
    assert_eq!(state.gold, 6);
}

#[test]
fn spell_807_tavern_dish_banana() {
    let (mut state, mut pool, mut rng) = setup_tavern(807);
    state.board.push(Unit::new("Target", 2, 2));
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
    assert_eq!(state.board[0].attack, 4);
    assert_eq!(state.board[0].health, 4);
}

#[test]
fn spell_808_them_apples() {
    let (mut state, mut pool, mut rng) = setup_tavern(808);
    state.shop.push(Unit::new("ShopMinion", 2, 2));
    state.add_to_hand(spells::spell_by_name("Them Apples").unwrap());
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
    assert_eq!(state.shop[0].attack, 3);
    assert_eq!(state.shop[0].health, 4);
}

// ============================================================================
// Tier 2 Tavern Spells (7)
// ============================================================================

#[test]
fn spell_809_chefs_choice() {
    let (mut state, mut pool, mut rng) = setup_tavern(809);
    state
        .board
        .push(tier1::glim_guardian::template().instantiate()); // Dragon
    state.add_to_hand(spells::spell_by_name("Chef's Choice").unwrap());
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
    assert!(state.hand[0].tribe.matches(Tribe::Dragon));
    assert_ne!(state.hand[0].card_id, tier1::glim_guardian::ID);
}

#[test]
fn spell_810_hasty_excavation() {
    let (mut state, mut pool, mut rng) = setup_tavern(810);
    state.shop.push(spells::spell_by_name("Hasty Excavation").unwrap()); // Costs 3 Health!
    let pre_hp = state.health;
    let pre_gold = state.gold;
    state
        .step(TavernAction::Buy { shop_index: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.health, pre_hp - 3);
    assert_eq!(state.gold, pre_gold);
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
    assert_eq!(state.gold, pre_gold + 1);
}

#[test]
fn spell_811_leaf_through_the_pages() {
    let (mut state, mut pool, mut rng) = setup_tavern(811);
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
    assert_eq!(state.auras.free_refreshes, 2);
}

#[test]
fn spell_812_might_of_stormwind() {
    let (mut state, mut pool, mut rng) = setup_tavern(812);
    state.board.push(Unit::new("M1", 1, 1));
    state.board.push(Unit::new("M2", 1, 1));
    state.add_to_hand(spells::spell_by_name("Might of Stormwind").unwrap());
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
    assert_eq!(state.board[0].health, 3);
    assert_eq!(state.board[1].attack, 2);
    assert_eq!(state.board[1].health, 3);
}

#[test]
fn spell_813_search_through_time() {
    let (mut state, mut pool, mut rng) = setup_tavern(813);
    state.add_to_hand(spells::spell_by_name("Search Through Time").unwrap());
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
    // Chosen minion is locked for 1 turn, then unlocks at Start of Turn!
    assert_eq!(state.hand[0].locked_turns, 1);
    state.start_turn(&mut pool, &mut rng);
    assert_eq!(state.hand[0].locked_turns, 0);
}

#[test]
fn spell_814_strike_oil() {
    let (mut state, mut pool, mut rng) = setup_tavern(814);
    let pre_max = state.max_gold;
    state.add_to_hand(spells::spell_by_name("Strike Oil").unwrap());
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
    assert_eq!(state.auras.base_max_gold_bonus, 1);
    assert_eq!(state.max_gold, pre_max + 1);
}

#[test]
fn spell_815_winners_bread() {
    let (mut state, mut pool, mut rng) = setup_tavern(815);
    state.board.push(Unit::new("Target", 2, 2));
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
    assert_eq!(state.board[0].attack, 4);
    assert_eq!(state.board[0].health, 5);
    assert_eq!(state.board[0].winners_bread_stacks, 1);
    // Win combat -> Start of Turn plays 1 Blood Gem on Target (+1/+1 -> 5/6)!
    state.last_combat_won = true;
    state.start_turn(&mut pool, &mut rng);
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 6);
}

// ============================================================================
// Tier 3 Tavern Spells (15)
// ============================================================================

#[test]
fn spell_816_careful_investment() {
    let (mut state, mut pool, mut rng) = setup_tavern(816);
    state.add_to_hand(spells::spell_by_name("Careful Investment").unwrap());
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
    assert_eq!(state.bonus_gold_next_turn, 2);
}

#[test]
fn spell_817_friendly_bounty() {
    let (mut state, mut pool, mut rng) = setup_tavern(817);
    state
        .board
        .push(tier1::glim_guardian::template().instantiate()); // Dragon
    state.add_to_hand(spells::spell_by_name("Friendly Bounty").unwrap());
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
    assert!(state.hand[0].tribe.matches(Tribe::Dragon));
}

#[test]
fn spell_818_healthy_bounty() {
    let (mut state, mut pool, mut rng) = setup_tavern(818);
    state.board.push(Unit::new("M1", 2, 2));
    state.add_to_hand(spells::spell_by_name("Healthy Bounty").unwrap());
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
    assert_eq!(state.board[0].health, 6);
}

#[test]
fn spell_819_hostile_bounty() {
    let (mut state, mut pool, mut rng) = setup_tavern(819);
    state.board.push(Unit::new("M1", 2, 2));
    state.add_to_hand(spells::spell_by_name("Hostile Bounty").unwrap());
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
    assert_eq!(state.board[0].health, 2);
}

#[test]
fn spell_820_overconfidence() {
    let (mut state, mut pool, mut rng) = setup_tavern(820);
    state.add_to_hand(spells::spell_by_name("Overconfidence").unwrap());
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
    assert_eq!(state.auras.effect_stacks(spells::SPELL_OVERCONFIDENCE), 1);
    state.board.push(Unit::new("Winner", 10, 10));
    let opp = vec![Unit::new("Loser", 1, 1)];
    let res = state.resolve_combat_against(&opp, 1, &Default::default(), &[], 820);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(state.bonus_gold_next_turn, 3);
}

#[test]
fn spell_821_planar_telescope() {
    let (mut state, mut pool, mut rng) = setup_tavern(821);
    state
        .board
        .push(tier1::glim_guardian::template().instantiate()); // Dragon
    state.add_to_hand(spells::spell_by_name("Planar Telescope").unwrap());
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
    assert!(state
        .discover_pending
        .as_ref()
        .unwrap()
        .iter()
        .all(|u| u.tribe.matches(Tribe::Dragon)));
}

#[test]
fn spell_822_repair_job() {
    let (mut state, mut pool, mut rng) = setup_tavern(822);
    state.board.push(Unit::new("Target", 2, 2));
    state.add_to_hand(spells::spell_by_name("Repair Job").unwrap());
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
    assert_eq!(state.board[0].health, 10);
}

#[test]
fn spell_823_robust_evolution() {
    let (mut state, mut pool, mut rng) = setup_tavern(823);
    let mut m = tier1::glim_guardian::template().instantiate();
    m.attack = 12;
    m.health = 15;
    state.board.push(m);
    state.add_to_hand(spells::spell_by_name("Robust Evolution").unwrap());
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
    assert_eq!(state.board[0].tavern_tier, 2);
    assert_eq!(state.board[0].attack, 12);
    assert_eq!(state.board[0].health, 15);
}

#[test]
fn spell_824_seafood_stew() {
    let (mut state, mut pool, mut rng) = setup_tavern(824);
    state.board.push(
        Unit::new("KwUnit", 2, 2)
            .with_keyword(Keyword::Taunt)
            .with_keyword(Keyword::DivineShield)
            .with_keyword(Keyword::Windfury),
    );
    state.add_to_hand(spells::spell_by_name("Seafood Stew").unwrap());
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
    // Base + 3 Bonus Keywords = 4 applications of +1/+1 = +4/+4 -> 6/6!
    assert_eq!(state.board[0].attack, 6);
    assert_eq!(state.board[0].health, 6);
}

#[test]
fn spell_825_selfish_bounty() {
    let (mut state, mut pool, mut rng) = setup_tavern(825);
    state.board.push(Unit::new("Leftmost", 2, 2));
    state.board.push(Unit::new("Other", 2, 2));
    state.add_to_hand(spells::spell_by_name("Selfish Bounty").unwrap());
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
    assert_eq!(state.board[0].attack, 8);
    assert_eq!(state.board[0].health, 8);
    assert_eq!(state.board[1].attack, 2);
}

#[test]
fn spell_826_shiny_ring() {
    let (mut state, mut pool, mut rng) = setup_tavern(826);
    state.board.push(Unit::new("M1", 2, 2));
    state.board.push(Unit::new("M2", 3, 3));
    state.add_to_hand(spells::spell_by_name("Shiny Ring").unwrap());
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
    assert_eq!(state.board[0].health, 3);
    assert_eq!(state.board[1].attack, 4);
    assert_eq!(state.board[1].health, 4);
}

#[test]
fn spell_827_staff_of_enrichment() {
    let (mut state, mut pool, mut rng) = setup_tavern(827);
    state.shop.push(Unit::new("ShopMinion", 2, 2));
    state.add_to_hand(spells::spell_by_name("Staff of Enrichment").unwrap());
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
    assert_eq!(state.auras.tavern_all_atk, 2);
    assert_eq!(state.auras.tavern_all_hp, 2);
    assert_eq!(state.shop[0].attack, 4);
    assert_eq!(state.shop[0].health, 4);
}

#[test]
fn spell_828_time_management() {
    let (mut state, mut pool, mut rng) = setup_tavern(828);
    state.board.push(Unit::new("M1", 2, 2));
    state.add_to_hand(spells::spell_by_name("Time Management").unwrap());
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
    // Option 1: Do It Later (+2/+2 twice at Start of Next Turn = +4/+4)
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 1 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.auras.effect_stacks(spells::SPELL_TIME_MANAGEMENT), 2);
    state.start_turn(&mut pool, &mut rng);
    assert_eq!(state.board[0].attack, 6);
    assert_eq!(state.board[0].health, 6);
}

#[test]
fn spell_829_tricky_trousers() {
    let (mut state, mut pool, mut rng) = setup_tavern(829);
    state.board.push(Unit::new("M1", 2, 2));
    state.add_to_hand(spells::spell_by_name("Tricky Trousers").unwrap());
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
    assert_eq!(state.board[0].health, 4);
    assert!(state.board[0].taunt);
}

#[test]
fn spell_830_wealthy_bounty() {
    let (mut state, mut pool, mut rng) = setup_tavern(830);
    state.gold = 5;
    state.add_to_hand(spells::spell_by_name("Wealthy Bounty").unwrap());
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
    assert_eq!(state.gold, 7);
}

// ============================================================================
// Tier 4 Tavern Spells (14 new + Gem Confiscation + Sludge Corrosion = 16)
// ============================================================================

#[test]
fn spell_831_blood_gem_barrage() {
    let (mut state, mut pool, mut rng) = setup_tavern(831);
    state.auras.blood_gem_bonus_atk = 1; // Blood Gems give +2/+1
    state.add_to_hand(spells::spell_by_name("Blood Gem Barrage").unwrap());
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
    assert_eq!(state.auras.refresh_blood_gems, 2);
    state
        .step(TavernAction::Refresh, &mut pool, &mut rng)
        .unwrap();
    assert!(
        state
            .shop
            .iter()
            .filter(|u| !u.is_spell)
            .all(|u| u.blood_gems_played == 2)
    );
}

#[test]
fn spell_832_boon_of_beetles() {
    let (mut state, mut pool, mut rng) = setup_tavern(832);
    state.add_to_hand(spells::spell_by_name("Boon of Beetles").unwrap());
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
    assert_eq!(state.auras.effect_stacks(spells::SPELL_BOON_OF_BEETLES), 2);

    // Verify combat summons 2 Taunt 2/2 Beetles when board has space!
    let gs = seaglass::GameState {
        auras_a: state.auras.clone(),
        ..Default::default()
    };
    let res = seaglass::simulate(&[], &[Unit::new("Dummy", 1, 1)], &gs, 832);
    assert_eq!(res.outcome, seaglass::BattleOutcome::AWin);
    assert_eq!(res.survivors_a.len(), 2);
    assert!(res.survivors_a.iter().all(|u| u.name == "Beetle" && u.taunt));
}

#[test]
fn spell_833_boundless_potential() {
    let (mut state, mut pool, mut rng) = setup_tavern(833);
    state.tavern_tier = 4;
    state.add_to_hand(spells::spell_by_name("Boundless Potential").unwrap());
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
    // Choose option 1: Discover a Tavern spell from your Tier (4)
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 1 },
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
    assert_eq!(state.hand[0].tavern_tier, 4);
}

#[test]
fn spell_834_cloning_conch() {
    let (mut state, mut pool, mut rng) = setup_tavern(834);
    state.add_to_hand(spells::spell_by_name("Cloning Conch").unwrap());
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
    assert!(state.hand[0].tribe.matches(Tribe::Murloc));
    assert_eq!(state.hand[0].card_id, state.hand[1].card_id);
}

#[test]
fn spell_835_defenders_rites() {
    let (mut state, mut pool, mut rng) = setup_tavern(835);
    state.board.push(Unit::new("Target", 2, 2));
    state.add_to_hand(spells::spell_by_name("Defender's Rites").unwrap());
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
    assert_eq!(state.board[0].attack, 9);
    assert_eq!(state.board[0].health, 9);
    assert!(state.board[0].taunt);
}

#[test]
fn spell_836_easterly_winds() {
    let (mut state, mut pool, mut rng) = setup_tavern(836);
    state.add_to_hand(spells::spell_by_name("Easterly Winds").unwrap());
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
    assert_eq!(state.auras.refresh_random_buffs, vec![(9, 9)]);
    state
        .step(TavernAction::Refresh, &mut pool, &mut rng)
        .unwrap();
    assert!(state
        .shop
        .iter()
        .any(|u| u.attack >= u.base_attack + 9 && u.health >= u.base_health + 9));
}

#[test]
fn spell_837_eonars_favor() {
    let (mut state, mut pool, mut rng) = setup_tavern(837);
    state
        .board
        .push(Unit::new("MyBeast", 2, 2).with_tribe(Tribe::Beast));
    state
        .shop
        .push(Unit::new("ShopBeast", 3, 3).with_tribe(Tribe::Beast));
    state
        .shop
        .push(Unit::new("ShopMech", 3, 3).with_tribe(Tribe::Mech));
    state.add_to_hand(spells::spell_by_name("Eonar's Favor").unwrap());
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
    assert_eq!(state.shop[0].attack, 6);
    assert_eq!(state.shop[0].health, 6);
    assert_eq!(state.shop[1].attack, 3);
    assert_eq!(state.shop[1].health, 3);
}

#[test]
fn spell_838_methodical_madness() {
    let (mut state, mut pool, mut rng) = setup_tavern(838);
    state
        .board
        .push(Unit::new("Target", 2, 2).with_tribe(Tribe::Demon));
    state
        .shop
        .push(Unit::new("S1", 3, 4).with_keyword(Keyword::Taunt));
    state
        .shop
        .push(Unit::new("S2", 5, 6).with_keyword(Keyword::DivineShield));
    state.add_to_hand(spells::spell_by_name("Methodical Madness").unwrap());
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
    assert!(state.shop.is_empty());
    assert_eq!(state.board[0].attack, 10);
    assert_eq!(state.board[0].health, 12);
    assert!(state.board[0].taunt);
    assert!(state.board[0].divine_shield);
}

#[test]
fn spell_839_mighty_dragonbreath() {
    let (mut state, mut pool, mut rng) = setup_tavern(839);
    state.board.push(
        Unit::new("DSDragon", 2, 2)
            .with_tribe(Tribe::Dragon)
            .with_keyword(Keyword::DivineShield),
    );
    state.add_to_hand(spells::spell_by_name("Mighty Dragonbreath").unwrap());
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
    // Base +3/+2 + Dragon +3/+2 + Divine Shield +3/+2 = +9/+6 -> 11/8!
    assert_eq!(state.board[0].attack, 11);
    assert_eq!(state.board[0].health, 8);
}

#[test]
fn spell_840_misplaced_tea_set() {
    let (mut state, mut pool, mut rng) = setup_tavern(840);
    state
        .board
        .push(Unit::new("Beast1", 1, 1).with_tribe(Tribe::Beast));
    state
        .board
        .push(Unit::new("Mech1", 1, 1).with_tribe(Tribe::Mech));
    state.add_to_hand(spells::spell_by_name("Misplaced Tea Set").unwrap());
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
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 5);
    assert_eq!(state.board[1].attack, 5);
    assert_eq!(state.board[1].health, 5);
}

#[test]
fn spell_841_natural_blessing() {
    let (mut state, mut pool, mut rng) = setup_tavern(841);
    state
        .board
        .push(Unit::new("Elem1", 1, 1).with_tribe(Tribe::Elemental));
    state
        .board
        .push(Unit::new("Elem2", 2, 2).with_tribe(Tribe::Elemental));
    state
        .shop
        .push(Unit::new("ShopElem", 3, 3).with_tribe(Tribe::Elemental));
    state.add_to_hand(spells::spell_by_name("Natural Blessing").unwrap());
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
    assert_eq!(state.board[1].attack, 4);
    assert_eq!(state.board[1].health, 3);
    assert_eq!(state.shop[0].attack, 5);
    assert_eq!(state.shop[0].health, 4);
}

#[test]
fn spell_842_temperature_shift() {
    let (mut state, mut pool, mut rng) = setup_tavern(842);
    state.add_to_hand(spells::spell_by_name("Temperature Shift").unwrap());
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
    assert_eq!(state.hand[0].card_id, tier2::fire_baller::ID);
    assert_eq!(state.hand[1].card_id, tier2::snow_baller::ID);
}

#[test]
fn spell_843_tomb_turning() {
    let (mut state, mut pool, mut rng) = setup_tavern(843);
    state.tavern_tier = 4;
    state.add_to_hand(spells::spell_by_name("Tomb Turning").unwrap());
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
    assert!(state.hand[0].dies_on_play_this_turn);

    // Replace with Plaguerunner with dies_on_play_this_turn = true and play it: it dies outside combat (+4 undead attack)!
    let mut pr = seaglass::cards::tier4::plaguerunner::template().instantiate();
    pr.dies_on_play_this_turn = true;
    state.hand[0] = pr;
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
    assert!(state.board.is_empty());
    assert_eq!(state.auras.undead_bonus_attack, 4);
}

#[test]
fn spell_844_weapons_forge() {
    let (mut state, mut pool, mut rng) = setup_tavern(844);
    state.add_to_hand(spells::spell_by_name("Weapons Forge").unwrap());
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
    assert_eq!(state.hand.len(), 3);
    assert!(state
        .hand
        .iter()
        .all(|u| u.card_id == tokens::SPELL_POINTY_ARROW));
}

// ============================================================================
// Generated Token Spells (8)
// ============================================================================

#[test]
fn token_spell_950_blood_gem() {
    let (mut state, mut pool, mut rng) = setup_tavern(950);
    state.auras.blood_gem_bonus_atk = 2;
    state.auras.blood_gem_bonus_hp = 1;
    state.board.push(Unit::new("Target", 2, 2));
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
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 4);
    assert_eq!(state.board[0].blood_gems_played, 1);
}

#[test]
fn token_spell_952_lockbox() {
    let (mut state, mut pool, mut rng) = setup_tavern(952);
    let mut box_card = tokens::make_lockbox();
    box_card.lockbox_turns_left = 1;
    state.add_to_hand(box_card);
    state.start_turn(&mut pool, &mut rng);
    assert_eq!(state.hand.len(), 1);
    assert!(!state.hand[0].is_spell);
    assert!(state.hand[0].is_golden);
}

#[test]
fn token_spell_953_gem_day() {
    let (mut state, mut pool, mut rng) = setup_tavern(953);
    state.add_to_hand(tokens::make_gem_day());
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
    assert_eq!(state.auras.blood_gem_bonus_atk, 1);
}

#[test]
fn token_spell_954_sludge_corrosion() {
    let (mut state, mut pool, mut rng) = setup_tavern(954);
    state.board.push(tier2::brain_rotter::template().instantiate());
    state.add_to_hand(tokens::make_sludge_corrosion());
    // Discarding Sludge Corrosion casts it twice (+1/+1 twice = +2/+2)!
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
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 6);
}

#[test]
fn token_spell_955_gem_confiscation() {
    let (mut state, mut pool, mut rng) = setup_tavern(955);
    state.board.push(Unit::new("Left", 3, 3));
    state.board.push(Unit::new("Mid", 2, 2));
    state.board[0].play_blood_gems(2, &state.auras); // Left is 5/5 with 2 Gems
    state.add_to_hand(tokens::make_gem_confiscation());
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
    // Left lost its 2 Gems (back to 3/3); Mid gained 3 Gems + 2 stolen Gems = 5 Gems (+5/+5 -> 7/7)!
    assert_eq!(state.board[0].attack, 3);
    assert_eq!(state.board[0].health, 3);
    assert_eq!(state.board[1].attack, 7);
    assert_eq!(state.board[1].health, 7);
    assert_eq!(state.board[1].blood_gems_played, 5);
}

#[test]
fn token_spell_956_golden_touch() {
    let (mut state, mut pool, mut rng) = setup_tavern(956);
    state
        .shop
        .push(tier3::fetid_corroder::template().instantiate()); // 3/3 plain in shop
    state.add_to_hand(tokens::make_golden_touch());
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
    assert!(state.shop[0].is_golden);
    assert_eq!(state.shop[0].attack, 6);
    assert_eq!(state.shop[0].health, 6);
}

#[test]
fn token_spell_957_pointy_arrow() {
    let (mut state, mut pool, mut rng) = setup_tavern(957);
    state.board.push(Unit::new("Target", 2, 2));
    state.add_to_hand(tokens::make_pointy_arrow());
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
    assert_eq!(state.board[0].health, 2);
}

#[test]
fn token_spell_958_arcane_absorption() {
    let (mut state, mut pool, mut rng) = setup_tavern(958);
    state
        .board
        .push(Unit::new("Target", 2, 2).with_tribe(Tribe::Elemental));
    state
        .shop
        .push(Unit::new("ShopElem", 6, 8).with_tribe(Tribe::Elemental));
    state.add_to_hand(tokens::make_arcane_absorption());
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
    // Gives half the stats (+3/+4) of the highest-Health minion in the Tavern!
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 6);
}

#[test]
fn token_spell_959_conflagration() {
    let (mut state, mut pool, mut rng) = setup_tavern(959);
    state.board.push(Unit::new("Target", 2, 2));
    state.add_to_hand(tokens::make_conflagration());
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
    assert_eq!(state.board[0].health, 6);
}

// ============================================================================
// Tier 5 Tavern Spells (14 new + Golden Touch = 15)
// ============================================================================

#[test]
fn spell_845_armor_stash() {
    let (mut state, mut pool, mut rng) = setup_tavern(845);
    state.armor = 0;
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
    assert_eq!(state.armor, 5);
}

#[test]
fn spell_846_brood_of_nozdormu() {
    let (mut state, mut pool, mut rng) = setup_tavern(846);
    state.board.push(Unit::new("Left", 10, 10));
    state.add_to_hand(spells::spell_by_name("Brood of Nozdormu").unwrap());
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
    let brood = spells::SPELL_BROOD_OF_NOZDORMU;
    assert_eq!(state.auras.effect_stacks(brood), 1);

    let gs = seaglass::GameState {
        auras_a: state.auras.clone(),
        ..Default::default()
    };
    let res = seaglass::simulate(&state.board, &[Unit::new("Enemy", 1, 1)], &gs, 846);
    assert_eq!(res.survivors_a[0].attack, 20);
}

#[test]
fn spell_847_butchering() {
    let (mut state, mut pool, mut rng) = setup_tavern(847);
    state
        .board
        .push(seaglass::cards::tier1::harmless_bonehead::template().instantiate());
    state.add_to_hand(spells::spell_by_name("Butchering").unwrap());
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
    assert_eq!(state.auras.undead_bonus_attack, 8);
    // Harmless Bonehead died and summoned two 1/1 Skeletons with +8 Attack (9/1)!
    assert_eq!(state.board.len(), 2);
    assert!(state.board.iter().all(|u| u.attack == 9 && u.health == 1));
}

#[test]
fn spell_848_channel_the_devourer() {
    let (mut state, mut pool, mut rng) = setup_tavern(848);
    state.gold = 0;
    state.board.push(Unit::new("Sacrifice", 6, 8));
    state.board.push(Unit::new("Receiver", 2, 2));
    state.add_to_hand(spells::spell_by_name("Channel the Devourer").unwrap());
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
    assert_eq!(state.gold, 1);
    assert_eq!(state.board.len(), 1);
    assert_eq!(state.board[0].attack, 8);
    assert_eq!(state.board[0].health, 10);
}

#[test]
fn spell_849_contracted_corpse() {
    let (mut state, mut pool, mut rng) = setup_tavern(849);
    state.tavern_tier = 5;
    state.add_to_hand(spells::spell_by_name("Contracted Corpse").unwrap());
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
    assert!(seaglass::cards::is_deathrattle_minion(state.hand[0].card_id));
}

#[test]
fn spell_850_corrupted_coin() {
    let (mut state, mut pool, mut rng) = setup_tavern(850);
    state.gold = 3;
    state.add_to_hand(spells::spell_by_name("Corrupted Coin").unwrap());
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
    assert_eq!(state.gold, 5);

    // Discarding Corrupted Coin increases max_gold by 2!
    let pre_max = state.max_gold;
    state.board.push(tier2::brain_rotter::template().instantiate());
    state.add_to_hand(spells::spell_by_name("Corrupted Coin").unwrap());
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
    assert_eq!(state.max_gold, pre_max + 2);
}

#[test]
fn spell_851_corrupted_cupcakes() {
    let (mut state, mut pool, mut rng) = setup_tavern(851);
    state
        .board
        .push(Unit::new("Demon1", 2, 2).with_tribe(Tribe::Demon));
    state.shop.push(Unit::new("S1", 2, 3));
    state.shop.push(Unit::new("S2", 3, 4));
    state.shop.push(Unit::new("S3", 4, 5));
    state.add_to_hand(spells::spell_by_name("Corrupted Cupcakes").unwrap());
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
    assert!(state.shop.is_empty());
    assert_eq!(state.board[0].attack, 11);
    assert_eq!(state.board[0].health, 14);
}

#[test]
fn spell_852_energizing_chamber() {
    let (mut state, mut pool, mut rng) = setup_tavern(852);
    state.add_to_hand(spells::spell_by_name("Energizing Chamber").unwrap());
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
    assert_eq!(state.auras.deity.attack, 8);
    assert_eq!(state.auras.deity.health, 8);

    // Discarding Energizing Chamber casts it twice (+14/+14) + Brain Rotter (+2/+2) = +16/+16 -> 24/24!
    state.board.push(tier2::brain_rotter::template().instantiate());
    state.add_to_hand(spells::spell_by_name("Energizing Chamber").unwrap());
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
    assert_eq!(state.auras.deity.attack, 24);
    assert_eq!(state.auras.deity.health, 24);
}

#[test]
fn spell_853_forests_bounty() {
    let (mut state, mut pool, mut rng) = setup_tavern(853);
    state.board.push(Unit::new("Target", 2, 2));
    state.add_to_hand(spells::spell_by_name("Forest's Bounty").unwrap());
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
    // Option 0: Give a minion +6/+6 twice (+12/+12 -> 14/14)
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.board[0].attack, 14);
    assert_eq!(state.board[0].health, 14);
}

#[test]
fn spell_854_hired_headhunter() {
    let (mut state, mut pool, mut rng) = setup_tavern(854);
    state.tavern_tier = 5;
    state.add_to_hand(spells::spell_by_name("Hired Headhunter").unwrap());
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
    assert!(seaglass::cards::is_battlecry_minion(state.hand[0].card_id));
}

#[test]
fn spell_855_saloons_finest() {
    let (mut state, mut pool, mut rng) = setup_tavern(855);
    state.tavern_tier = 5;
    state.shop.push(Unit::new("Minion", 1, 1));
    state.add_to_hand(spells::spell_by_name("Saloon's Finest").unwrap());
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
    assert!(!state.shop.is_empty());
    assert!(state.shop.iter().all(|u| u.is_spell));
}

#[test]
fn spell_856_unmasked_identity() {
    let (mut state, mut pool, mut rng) = setup_tavern(856);
    state.add_to_hand(spells::spell_by_name("Unmasked Identity").unwrap());
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
            TavernAction::ChooseDiscover { option_index: 1 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.auras.hero_power_id, 2);
}

#[test]
fn spell_857_upper_hand() {
    let (mut state, mut pool, mut rng) = setup_tavern(857);
    state.add_to_hand(spells::spell_by_name("Upper Hand").unwrap());
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
    assert_eq!(state.auras.effect_stacks(spells::SPELL_UPPER_HAND), 1);

    let gs = seaglass::GameState {
        auras_a: state.auras.clone(),
        ..Default::default()
    };
    let res = seaglass::simulate(
        &[Unit::new("MyUnit", 2, 2)],
        &[Unit::new("BigEnemy", 1, 50)],
        &gs,
        857,
    );
    assert_eq!(res.outcome, seaglass::BattleOutcome::AWin);
}

#[test]
fn spell_858_wave_of_gold() {
    let (mut state, mut pool, mut rng) = setup_tavern(858);
    state.board.push(Unit::new("Plain", 2, 2));
    let mut gold = Unit::new("Gold", 4, 4);
    gold.is_golden = true;
    state.board.push(gold);
    state.add_to_hand(spells::spell_by_name("Wave of Gold").unwrap());
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
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 4);
    assert_eq!(state.board[1].attack, 10);
    assert_eq!(state.board[1].health, 8);
}

// ============================================================================
// Tier 6 Tavern Spells (5)
// ============================================================================

#[test]
fn spell_859_azerite_empowerment() {
    let (mut state, mut pool, mut rng) = setup_tavern(859);
    state.board.push(Unit::new("M1", 2, 2));
    state.board.push(Unit::new("M2", 3, 3));
    state.add_to_hand(spells::spell_by_name("Azerite Empowerment").unwrap());
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
    // +2/+2 twice = +4/+4 to all friendly minions
    assert_eq!(state.board[0].attack, 6);
    assert_eq!(state.board[0].health, 6);
    assert_eq!(state.board[1].attack, 7);
    assert_eq!(state.board[1].health, 7);
}

#[test]
fn spell_860_eyes_of_the_earth_mother() {
    let (mut state, mut pool, mut rng) = setup_tavern(860);
    state.gold = 10;
    state
        .board
        .push(seaglass::cards::tier4::conveyor_construct::template().instantiate()); // Tier 4 (5/2)
    state.add_to_hand(spells::spell_by_name("Eyes of the Earth Mother").unwrap());
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
    assert!(state.board[0].is_golden);
    assert_eq!(state.board[0].attack, 10);
    assert_eq!(state.board[0].health, 4);
}

#[test]
fn spell_861_fandrals_fortune() {
    let (mut state, mut pool, mut rng) = setup_tavern(861);
    state.tavern_tier = 6;
    state.board.push(Unit::new("Target", 2, 2));
    state.add_to_hand(spells::spell_by_name("Fandral's Fortune").unwrap());
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
    assert!(state.hand[0].fandral_combined);

    // Replace with Alliance Flag with fandral_combined = true and play it: both options (+3/+1 and +1/+3 = +4/+4) apply without prompting!
    let mut flag = spells::spell_by_name("Alliance Flag").unwrap();
    flag.fandral_combined = true;
    state.hand[0] = flag;
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
    assert_eq!(state.board[0].attack, 6);
    assert_eq!(state.board[0].health, 6);
}

#[test]
fn spell_862_lost_staff_of_hamuul() {
    let (mut state, mut pool, mut rng) = setup_tavern(862);
    state.tavern_tier = 6;
    state
        .board
        .push(Unit::new("MyDragon", 2, 2).with_tribe(Tribe::Dragon));
    state.add_to_hand(spells::spell_by_name("Lost Staff of Hamuul").unwrap());
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
    assert!(!state.shop.is_empty());
    assert!(
        state
            .shop
            .iter()
            .filter(|u| !u.is_spell)
            .all(|u| u.tribe.matches(Tribe::Dragon))
    );
}

#[test]
fn spell_863_perfect_vision() {
    let (mut state, mut pool, mut rng) = setup_tavern(863);
    state.board.push(Unit::new("Small", 1, 1));
    state.add_to_hand(spells::spell_by_name("Perfect Vision").unwrap());
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
    assert_eq!(state.board[0].attack, 20);
    assert_eq!(state.board[0].health, 20);
}

// ============================================================================
// Tier 7 Tavern Spells (4)
// ============================================================================

#[test]
fn spell_864_hallowed_ritual() {
    let (mut state, mut pool, mut rng) = setup_tavern(864);
    assert_eq!(spells::tier7_spells().len(), 4);
    assert_eq!(spells::spells_up_to_tier(7).len(), 70);

    state.add_to_hand(spells::spell_by_name("Hallowed Ritual").unwrap());
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
    assert!(
        state
            .discover_pending
            .as_ref()
            .unwrap()
            .iter()
            .all(|u| u.tavern_tier == 7)
    );
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].tavern_tier, 7);
}

#[test]
fn spell_865_menagerie_tableware() {
    let (mut state, mut pool, mut rng) = setup_tavern(865);
    state
        .board
        .push(Unit::new("B1", 2, 2).with_tribe(Tribe::Beast));
    state
        .board
        .push(Unit::new("B2", 2, 2).with_tribe(Tribe::Beast));
    state
        .board
        .push(Unit::new("D1", 3, 3).with_tribe(Tribe::Dragon));
    state.add_to_hand(spells::spell_by_name("Menagerie Tableware").unwrap());
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
    // 2 distinct friendly minion types (Beast, Dragon) -> repeats = 1 + 2 = 3 times (+9/+9 to all minions)!
    assert_eq!(state.board[0].attack, 11);
    assert_eq!(state.board[0].health, 11);
    assert_eq!(state.board[1].attack, 11);
    assert_eq!(state.board[1].health, 11);
    assert_eq!(state.board[2].attack, 12);
    assert_eq!(state.board[2].health, 12);
}

#[test]
fn spell_866_sacred_gift() {
    let (mut state, mut pool, mut rng) = setup_tavern(866);
    state.board.push(Unit::new("Target", 4, 5));
    state.add_to_hand(spells::spell_by_name("Sacred Gift").unwrap());
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
}

#[test]
fn spell_867_sharing_is_caring() {
    let (mut state, mut pool, mut rng) = setup_tavern(867);
    state.board.push(Unit::new("Left", 3, 4));
    state.board.push(Unit::new("Right", 2, 2));
    state.add_to_hand(spells::spell_by_name("Sharing is Caring").unwrap());
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
    let sharing = spells::SPELL_SHARING_IS_CARING;
    assert_eq!(state.auras.effect_stacks(sharing), 1);

    let gs = seaglass::GameState {
        auras_a: state.auras.clone(),
        ..Default::default()
    };
    let opp = vec![
        Unit::new("NearestEnemy", 15, 15),
        Unit::new("FarEnemy", 1, 1),
    ];
    let res = seaglass::simulate(&state.board, &opp, &gs, 867);
    // Left-most minion (3/4) gained stats of nearest enemy (15/15) -> 18/19 at Start of Combat!
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a[0].attack, 18);
}
