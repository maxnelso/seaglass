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
    assert_eq!(state.auras.overconfidence_stacks, 1);
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
    assert_eq!(state.auras.time_management_next_turn, 2);
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
// Generated Token Spells (6)
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
