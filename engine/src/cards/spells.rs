//! Tier 1 and Tier 2 Tavern Spells (`Patch 36.6.3`).

use crate::cards::tokens::{
    self, make_choice_option, CHOICE_ALLIANCE_ATK, CHOICE_ALLIANCE_HP, CHOICE_GEM_DAY_ATK,
    CHOICE_GEM_DAY_HP, SPELL_BLOOD_GEM, SPELL_GEM_DAY, SPELL_TAVERN_COIN,
};
use crate::model::{CardId, Keyword, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

// Tier 1 Tavern Spells (8)
pub const SPELL_A_NEW_SPROUT: CardId = 801;
pub const SPELL_ALLIANCE_FLAG: CardId = 802;
pub const SPELL_ENCHANTED_LASSO: CardId = 803;
pub const SPELL_FORTIFY: CardId = 804;
pub const SPELL_RECRUIT_A_TRAINEE: CardId = 805;
// SPELL_TAVERN_COIN = 951 (defined in `tokens.rs`)
pub const SPELL_TAVERN_DISH_BANANA: CardId = 807;
pub const SPELL_THEM_APPLES: CardId = 808;

// Tier 2 Tavern Spells (7)
pub const SPELL_CHEFS_CHOICE: CardId = 809;
pub const SPELL_HASTY_EXCAVATION: CardId = 810;
pub const SPELL_LEAF_THROUGH_THE_PAGES: CardId = 811;
pub const SPELL_MIGHT_OF_STORMWIND: CardId = 812;
pub const SPELL_SEARCH_THROUGH_TIME: CardId = 813;
pub const SPELL_STRIKE_OIL: CardId = 814;
pub const SPELL_WINNERS_BREAD: CardId = 815;

fn make_tavern_spell(card_id: CardId, name: &str, tier: u32, cost: u32, costs_health: bool) -> Unit {
    let mut u = Unit::new(name, 0, 0)
        .with_card_id(card_id)
        .with_tavern_tier(tier);
    u.is_spell = true;
    u.spell_cost = cost;
    u.costs_health = costs_health;
    u
}

/// All 8 active Tier 1 Tavern Spells (Patch 36.6.3).
pub fn tier1_spells() -> Vec<Unit> {
    vec![
        make_tavern_spell(SPELL_A_NEW_SPROUT, "A New Sprout", 1, 3, false),
        make_tavern_spell(SPELL_ALLIANCE_FLAG, "Alliance Flag", 1, 1, false),
        make_tavern_spell(SPELL_ENCHANTED_LASSO, "Enchanted Lasso", 1, 2, false),
        make_tavern_spell(SPELL_FORTIFY, "Fortify", 1, 1, false),
        make_tavern_spell(SPELL_RECRUIT_A_TRAINEE, "Recruit a Trainee", 1, 2, false),
        tokens::make_tavern_coin(),
        make_tavern_spell(SPELL_TAVERN_DISH_BANANA, "Tavern Dish Banana", 1, 1, false),
        make_tavern_spell(SPELL_THEM_APPLES, "Them Apples", 1, 1, false),
    ]
}

/// All 7 active Tier 2 Tavern Spells (Patch 36.6.3).
pub fn tier2_spells() -> Vec<Unit> {
    vec![
        make_tavern_spell(SPELL_CHEFS_CHOICE, "Chef's Choice", 2, 2, false),
        make_tavern_spell(SPELL_HASTY_EXCAVATION, "Hasty Excavation", 2, 3, true),
        make_tavern_spell(
            SPELL_LEAF_THROUGH_THE_PAGES,
            "Leaf Through the Pages",
            2,
            1,
            false,
        ),
        make_tavern_spell(SPELL_MIGHT_OF_STORMWIND, "Might of Stormwind", 2, 2, false),
        make_tavern_spell(
            SPELL_SEARCH_THROUGH_TIME,
            "Search Through Time",
            2,
            2,
            false,
        ),
        make_tavern_spell(SPELL_STRIKE_OIL, "Strike Oil", 2, 3, false),
        make_tavern_spell(SPELL_WINNERS_BREAD, "Winner's Bread", 2, 2, false),
    ]
}

/// Return all Tavern Spells with `tavern_tier <= max_tier`.
pub fn spells_up_to_tier(max_tier: u32) -> Vec<Unit> {
    let mut list = tier1_spells();
    if max_tier >= 2 {
        list.extend(tier2_spells());
    }
    list
}

/// Look up a Tavern Spell or token spell by exact name.
pub fn spell_by_name(name: &str) -> Option<Unit> {
    match name {
        "Blood Gem" => return Some(tokens::make_blood_gem()),
        "Lockbox" => return Some(tokens::make_lockbox()),
        "Gem Day" => return Some(tokens::make_gem_day()),
        _ => {}
    }
    spells_up_to_tier(6).into_iter().find(|s| s.name == name)
}

/// Draw a uniformly random Tavern Spell with `tavern_tier <= max_tier`.
pub fn draw_random_tavern_spell(max_tier: u32, rng: &mut Rng) -> Unit {
    let pool = spells_up_to_tier(max_tier);
    let idx = rng.below(pool.len());
    pool[idx].clone()
}

/// Draw up to `count` distinct Tavern Spells with `tavern_tier <= max_tier` for Discover.
pub fn draw_discover_tavern_spells(max_tier: u32, count: usize, rng: &mut Rng) -> Vec<Unit> {
    let mut pool = spells_up_to_tier(max_tier);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        if pool.is_empty() {
            break;
        }
        let idx = if pool.len() == 1 {
            0
        } else {
            rng.below(pool.len())
        };
        out.push(pool.remove(idx));
    }
    out
}

/// Returns `true` if casting `card_id` from hand requires targeting a friendly minion on `board`.
pub fn spell_requires_board_target(card_id: CardId) -> bool {
    matches!(
        card_id,
        SPELL_BLOOD_GEM
            | SPELL_ALLIANCE_FLAG
            | SPELL_FORTIFY
            | SPELL_TAVERN_DISH_BANANA
            | SPELL_CHEFS_CHOICE
            | SPELL_WINNERS_BREAD
    )
}

/// Resolve the effect of casting `card` from `hand` at `board_pos`.
pub fn cast_spell(
    state: &mut TavernState,
    card: Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    match card.card_id {
        SPELL_BLOOD_GEM => {
            if board_pos < state.board.len() {
                state.board[board_pos].play_blood_gems(1, &state.auras);
            }
        }
        SPELL_TAVERN_COIN | SPELL_HASTY_EXCAVATION => {
            state.gold = (state.gold + 1).min(10);
        }
        SPELL_GEM_DAY => {
            let opts = vec![
                make_choice_option(CHOICE_GEM_DAY_ATK, "Gem Day (+1 Attack)", false),
                make_choice_option(CHOICE_GEM_DAY_HP, "Gem Day (+1 Health)", false),
            ];
            state.push_discover(opts);
        }
        SPELL_A_NEW_SPROUT => {
            let opts = pool.draw_discover_options(1, 3, rng);
            if !opts.is_empty() {
                state.push_discover(opts);
            }
        }
        SPELL_ALLIANCE_FLAG => {
            if board_pos < state.board.len() {
                state.pending_choice_target = Some(board_pos);
                let opts = vec![
                    make_choice_option(CHOICE_ALLIANCE_ATK, "Alliance Flag (+3/+1)", false),
                    make_choice_option(CHOICE_ALLIANCE_HP, "Alliance Flag (+1/+3)", false),
                ];
                state.push_discover(opts);
            }
        }
        SPELL_ENCHANTED_LASSO => {
            let shop_minions: Vec<usize> = state
                .shop
                .iter()
                .enumerate()
                .filter(|(_, u)| !u.is_spell)
                .map(|(i, _)| i)
                .collect();
            if !shop_minions.is_empty() && state.hand.len() < 10 {
                let pick = if shop_minions.len() == 1 {
                    shop_minions[0]
                } else {
                    shop_minions[rng.below(shop_minions.len())]
                };
                let stolen = state.shop.remove(pick);
                state.add_to_hand(stolen);
            }
        }
        SPELL_FORTIFY => {
            if board_pos < state.board.len() {
                let (atk, hp) = state.auras.spell_stat_buff(0, 3);
                state.board[board_pos].add_stats(atk, hp);
                state.board[board_pos].apply_keyword(Keyword::Taunt, false);
            }
        }
        SPELL_RECRUIT_A_TRAINEE => {
            if state.hand.len() < 10 {
                if let Some(mut drawn) = pool.draw_from_pool(1, rng) {
                    state.apply_global_unit_auras(&mut drawn);
                    state.add_to_hand(drawn);
                }
            }
        }
        SPELL_TAVERN_DISH_BANANA => {
            if board_pos < state.board.len() {
                let (atk, hp) = state.auras.spell_stat_buff(2, 2);
                state.board[board_pos].add_stats(atk, hp);
            }
        }
        SPELL_THEM_APPLES => {
            let (atk, hp) = state.auras.spell_stat_buff(1, 2);
            for u in &mut state.shop {
                if !u.is_spell {
                    u.add_stats(atk, hp);
                }
            }
        }
        SPELL_CHEFS_CHOICE => {
            if board_pos < state.board.len() && state.hand.len() < 10 {
                let target_tribe = state.board[board_pos].tribe;
                let exclude_id = state.board[board_pos].card_id;
                if let Some(mut drawn) =
                    pool.draw_by_tribe(target_tribe, Some(exclude_id), state.tavern_tier, rng)
                {
                    state.apply_global_unit_auras(&mut drawn);
                    state.add_to_hand(drawn);
                }
            }
        }
        SPELL_LEAF_THROUGH_THE_PAGES => {
            state.auras.free_refreshes += 2;
        }
        SPELL_MIGHT_OF_STORMWIND => {
            if !state.board.is_empty() {
                let (atk, hp) = state.auras.spell_stat_buff(1, 2);
                let mut indices: Vec<usize> = (0..state.board.len()).collect();
                let count = indices.len().min(4);
                for _ in 0..count {
                    let pick = if indices.len() == 1 {
                        0
                    } else {
                        rng.below(indices.len())
                    };
                    let b_idx = indices.remove(pick);
                    state.board[b_idx].add_stats(atk, hp);
                }
            }
        }
        SPELL_SEARCH_THROUGH_TIME => {
            let mut opts = pool.draw_discover_options(state.tavern_tier, 3, rng);
            for opt in &mut opts {
                state.apply_global_unit_auras(opt);
                opt.locked_turns = 1;
            }
            if !opts.is_empty() {
                state.push_discover(opts);
            }
        }
        SPELL_STRIKE_OIL => {
            state.auras.base_max_gold_bonus += 1;
            state.max_gold += 1;
        }
        SPELL_WINNERS_BREAD => {
            if board_pos < state.board.len() {
                let (atk, hp) = state.auras.spell_stat_buff(2, 3);
                state.board[board_pos].add_stats(atk, hp);
                state.board[board_pos].winners_bread_stacks += 1;
            }
        }
        _ => {
            if board_pos < state.board.len() {
                state.board[board_pos].add_stats(card.attack, card.health);
            }
        }
    }
}
