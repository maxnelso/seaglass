//! Tier 1, Tier 2, and Tier 3 Tavern Spells (`Patch 36.6.3`).

use crate::cards::tokens::{
    self, is_choice_option, make_choice_option, CHOICE_ALLIANCE_ATK, CHOICE_ALLIANCE_HP,
    CHOICE_GEM_DAY_ATK, CHOICE_GEM_DAY_HP, CHOICE_TIME_MGMT_LATER, CHOICE_TIME_MGMT_NOW,
    SPELL_BLOOD_GEM, SPELL_GEM_CONFISCATION, SPELL_GEM_DAY, SPELL_GOLDEN_TOUCH,
    SPELL_SLUDGE_CORROSION, SPELL_TAVERN_COIN,
};
use crate::model::{CardId, Keyword, Tribe, Unit, BONUS_KEYWORDS, SINGLE_TRIBES};
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

// Tier 3 Tavern Spells (15)
pub const SPELL_CAREFUL_INVESTMENT: CardId = 816;
pub const SPELL_FRIENDLY_BOUNTY: CardId = 817;
pub const SPELL_HEALTHY_BOUNTY: CardId = 818;
pub const SPELL_HOSTILE_BOUNTY: CardId = 819;
pub const SPELL_OVERCONFIDENCE: CardId = 820;
pub const SPELL_PLANAR_TELESCOPE: CardId = 821;
pub const SPELL_REPAIR_JOB: CardId = 822;
pub const SPELL_ROBUST_EVOLUTION: CardId = 823;
pub const SPELL_SEAFOOD_STEW: CardId = 824;
pub const SPELL_SELFISH_BOUNTY: CardId = 825;
pub const SPELL_SHINY_RING: CardId = 826;
pub const SPELL_STAFF_OF_ENRICHMENT: CardId = 827;
pub const SPELL_TIME_MANAGEMENT: CardId = 828;
pub const SPELL_TRICKY_TROUSERS: CardId = 829;
pub const SPELL_WEALTHY_BOUNTY: CardId = 830;

fn make_tavern_spell(card_id: CardId, name: &str, tier: u32, cost: u32, costs_health: bool) -> Unit {
    let mut u = Unit::new(name, 0, 0)
        .with_card_id(card_id)
        .with_tavern_tier(tier);
    u.is_spell = true;
    u.spell_cost = cost;
    u.costs_health = costs_health;
    u
}

/// Construct a `Fortify` spell card (`+3 Health and Taunt`).
pub fn make_fortify() -> Unit {
    make_tavern_spell(SPELL_FORTIFY, "Fortify", 1, 1, false)
}

/// Construct a `Tavern Dish Banana` spell card (`+2/+2`).
pub fn make_tavern_dish_banana() -> Unit {
    make_tavern_spell(SPELL_TAVERN_DISH_BANANA, "Tavern Dish Banana", 1, 1, false)
}

/// Construct a `Repair Job` spell card (`+4/+8`).
pub fn make_repair_job() -> Unit {
    make_tavern_spell(SPELL_REPAIR_JOB, "Repair Job", 3, 2, false)
}

/// Returns `true` if `card_id` is a Tavern spell (`spellSchool: TAVERN`, triggering `Timecap'n Hooktail` / `Vicious Mindslasher`).
pub fn is_tavern_spell(card_id: CardId) -> bool {
    card_id != SPELL_BLOOD_GEM
        && card_id != tokens::SPELL_LOCKBOX
        && !is_choice_option(card_id)
}

/// All 8 active Tier 1 Tavern Spells (Patch 36.6.3).
pub fn tier1_spells() -> Vec<Unit> {
    vec![
        make_tavern_spell(SPELL_A_NEW_SPROUT, "A New Sprout", 1, 3, false),
        make_tavern_spell(SPELL_ALLIANCE_FLAG, "Alliance Flag", 1, 1, false),
        make_tavern_spell(SPELL_ENCHANTED_LASSO, "Enchanted Lasso", 1, 2, false),
        make_fortify(),
        make_tavern_spell(SPELL_RECRUIT_A_TRAINEE, "Recruit a Trainee", 1, 2, false),
        tokens::make_tavern_coin(),
        make_tavern_dish_banana(),
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

/// All 15 active Tier 3 Tavern Spells (Patch 36.6.3).
pub fn tier3_spells() -> Vec<Unit> {
    vec![
        make_tavern_spell(SPELL_CAREFUL_INVESTMENT, "Careful Investment", 3, 1, false),
        make_tavern_spell(SPELL_FRIENDLY_BOUNTY, "Friendly Bounty", 3, 2, false),
        make_tavern_spell(SPELL_HEALTHY_BOUNTY, "Healthy Bounty", 3, 2, false),
        make_tavern_spell(SPELL_HOSTILE_BOUNTY, "Hostile Bounty", 3, 2, false),
        make_tavern_spell(SPELL_OVERCONFIDENCE, "Overconfidence", 3, 1, false),
        make_tavern_spell(SPELL_PLANAR_TELESCOPE, "Planar Telescope", 3, 4, false),
        make_repair_job(),
        make_tavern_spell(SPELL_ROBUST_EVOLUTION, "Robust Evolution", 3, 1, false),
        make_tavern_spell(SPELL_SEAFOOD_STEW, "Seafood Stew", 3, 2, false),
        make_tavern_spell(SPELL_SELFISH_BOUNTY, "Selfish Bounty", 3, 2, false),
        make_tavern_spell(SPELL_SHINY_RING, "Shiny Ring", 3, 2, false),
        make_tavern_spell(
            SPELL_STAFF_OF_ENRICHMENT,
            "Staff of Enrichment",
            3,
            2,
            false,
        ),
        make_tavern_spell(SPELL_TIME_MANAGEMENT, "Time Management", 3, 4, false),
        make_tavern_spell(SPELL_TRICKY_TROUSERS, "Tricky Trousers", 3, 1, false),
        make_tavern_spell(SPELL_WEALTHY_BOUNTY, "Wealthy Bounty", 3, 2, false),
    ]
}

/// Return all Tavern Spells with `tavern_tier <= max_tier`.
pub fn spells_up_to_tier(max_tier: u32) -> Vec<Unit> {
    let mut list = tier1_spells();
    if max_tier >= 2 {
        list.extend(tier2_spells());
    }
    if max_tier >= 3 {
        list.extend(tier3_spells());
    }
    list
}

/// Look up a Tavern Spell or token spell by exact name.
pub fn spell_by_name(name: &str) -> Option<Unit> {
    match name {
        "Blood Gem" => return Some(tokens::make_blood_gem()),
        "Lockbox" => return Some(tokens::make_lockbox()),
        "Gem Day" => return Some(tokens::make_gem_day()),
        "Sludge Corrosion" => return Some(tokens::make_sludge_corrosion()),
        "Gem Confiscation" => return Some(tokens::make_gem_confiscation()),
        "Golden Touch" => return Some(tokens::make_golden_touch()),
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

/// Draw a uniformly random Tavern Spell with exact `cost` (e.g. 1-Cost for `Unwilling Slacker`, 2-Cost for `Blue Chromadrake`).
pub fn draw_random_cost_tavern_spell(cost: u32, rng: &mut Rng) -> Unit {
    let pool: Vec<Unit> = spells_up_to_tier(6)
        .into_iter()
        .filter(|s| s.spell_cost == cost && !s.costs_health)
        .collect();
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

/// Determine the player's most common minion tribe on `board` (`Friendly Bounty`, `Planar Telescope`).
/// Ties are broken uniformly at random using `rng`.
pub fn most_common_tribe(board: &[Unit], rng: &mut Rng) -> Tribe {
    let mut best_count = 0usize;
    let mut candidates = Vec::new();
    for &tribe in &SINGLE_TRIBES {
        let count = board.iter().filter(|u| u.tribe.matches(tribe)).count();
        if count > best_count {
            best_count = count;
            candidates.clear();
            candidates.push(tribe);
        } else if count == best_count {
            candidates.push(tribe);
        }
    }
    if candidates.len() == 1 {
        candidates[0]
    } else {
        candidates[rng.below(candidates.len())]
    }
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
            | SPELL_REPAIR_JOB
            | SPELL_ROBUST_EVOLUTION
            | SPELL_SEAFOOD_STEW
            | SPELL_TRICKY_TROUSERS
            | SPELL_GEM_CONFISCATION
    )
}

fn buff_random_friendly(
    state: &mut TavernState,
    count: usize,
    base_atk: i32,
    base_hp: i32,
    rng: &mut Rng,
) {
    if state.board.is_empty() {
        return;
    }
    let (atk, hp) = state.auras.spell_stat_buff(base_atk, base_hp);
    let mut indices: Vec<usize> = (0..state.board.len()).collect();
    let n = indices.len().min(count);
    for _ in 0..n {
        let pick = if indices.len() == 1 {
            0
        } else {
            rng.below(indices.len())
        };
        let b_idx = indices.remove(pick);
        state.board[b_idx].add_stats(atk, hp);
    }
}

/// Resolve the effect of casting `card` from `hand` at `board_pos`.
pub fn cast_spell(
    state: &mut TavernState,
    card: Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if is_tavern_spell(card.card_id) {
        crate::cards::on_cast_tavern_spell(state);
    }

    match card.card_id {
        SPELL_BLOOD_GEM => {
            if board_pos < state.board.len() {
                state.board[board_pos].play_blood_gems(1, &state.auras);
            }
        }
        SPELL_TAVERN_COIN | SPELL_HASTY_EXCAVATION => {
            let cap = 10 + state.auras.base_max_gold_bonus;
            state.gold = (state.gold + 1).min(cap);
        }
        SPELL_GEM_DAY => {
            let opt0 = make_choice_option(CHOICE_GEM_DAY_ATK, "Gem Day (+1 Attack)", false);
            let opt1 = make_choice_option(CHOICE_GEM_DAY_HP, "Gem Day (+1 Health)", false);
            state.resolve_choose_one(opt0, opt1, pool, rng);
        }
        SPELL_A_NEW_SPROUT => {
            let mut opts = pool.draw_discover_options(1, 3, rng);
            for opt in &mut opts {
                state.apply_global_unit_auras(opt);
            }
            if !opts.is_empty() {
                state.push_discover(opts);
            }
        }
        SPELL_ALLIANCE_FLAG => {
            if board_pos < state.board.len() {
                state.pending_choice_target = Some(board_pos);
                let opt0 = make_choice_option(CHOICE_ALLIANCE_ATK, "Alliance Flag (+3/+1)", false);
                let opt1 = make_choice_option(CHOICE_ALLIANCE_HP, "Alliance Flag (+1/+3)", false);
                state.resolve_choose_one(opt0, opt1, pool, rng);
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
            buff_random_friendly(state, 4, 1, 2, rng);
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
        SPELL_CAREFUL_INVESTMENT => {
            state.bonus_gold_next_turn += 2;
        }
        SPELL_FRIENDLY_BOUNTY => {
            if state.hand.len() < 10 {
                let tribe = most_common_tribe(&state.board, rng);
                if let Some(mut drawn) = pool.draw_by_tribe(tribe, None, state.tavern_tier, rng) {
                    state.apply_global_unit_auras(&mut drawn);
                    state.add_to_hand(drawn);
                }
            }
        }
        SPELL_HEALTHY_BOUNTY => {
            buff_random_friendly(state, 4, 0, 4, rng);
        }
        SPELL_HOSTILE_BOUNTY => {
            buff_random_friendly(state, 4, 4, 0, rng);
        }
        SPELL_OVERCONFIDENCE => {
            state.auras.overconfidence_stacks += 1;
        }
        SPELL_PLANAR_TELESCOPE => {
            let tribe = most_common_tribe(&state.board, rng);
            let mut opts = pool.draw_discover_by_tribe(tribe, state.tavern_tier, 3, rng);
            for opt in &mut opts {
                state.apply_global_unit_auras(opt);
            }
            if !opts.is_empty() {
                state.push_discover(opts);
            }
        }
        SPELL_REPAIR_JOB => {
            if board_pos < state.board.len() {
                let (atk, hp) = state.auras.spell_stat_buff(4, 8);
                state.board[board_pos].add_stats(atk, hp);
            }
        }
        SPELL_ROBUST_EVOLUTION => {
            if board_pos < state.board.len() {
                let old = state.board[board_pos].clone();
                let target_tier = (old.tavern_tier + 1).min(6);
                let mut opts = pool.draw_discover_options(target_tier, 1, rng);
                if let Some(mut evolved) = opts.pop() {
                    pool.return_unit(&old);
                    evolved.attack = old.attack;
                    evolved.health = old.health;
                    evolved.max_attack = old.attack.max(evolved.base_attack);
                    evolved.max_health = old.health.max(evolved.base_health);
                    crate::cards::check_stat_thresholds(&mut evolved);
                    state.board[board_pos] = evolved;
                }
            }
        }
        SPELL_SEAFOOD_STEW => {
            if board_pos < state.board.len() {
                let kw_count = BONUS_KEYWORDS
                    .iter()
                    .filter(|&&kw| state.board.iter().any(|u| u.has_keyword(kw)))
                    .count();
                let repeats = (1 + kw_count) as i32;
                let (atk, hp) = state.auras.spell_stat_buff(1, 1);
                state.board[board_pos].add_stats(atk * repeats, hp * repeats);
            }
        }
        SPELL_SELFISH_BOUNTY => {
            if !state.board.is_empty() {
                let (atk, hp) = state.auras.spell_stat_buff(6, 6);
                state.board[0].add_stats(atk, hp);
            }
        }
        SPELL_SHINY_RING | SPELL_SLUDGE_CORROSION => {
            let (atk, hp) = state.auras.spell_stat_buff(1, 1);
            for u in &mut state.board {
                u.add_stats(atk, hp);
            }
        }
        SPELL_STAFF_OF_ENRICHMENT => {
            let (atk, hp) = state.auras.spell_stat_buff(2, 2);
            state.auras.tavern_all_atk += atk;
            state.auras.tavern_all_hp += hp;
            for u in &mut state.shop {
                if !u.is_spell {
                    u.add_stats(atk, hp);
                }
            }
        }
        SPELL_TIME_MANAGEMENT => {
            let opt0 = make_choice_option(CHOICE_TIME_MGMT_NOW, "Hurry Up (+2/+2 now)", false);
            let opt1 = make_choice_option(
                CHOICE_TIME_MGMT_LATER,
                "Do It Later (+2/+2 twice next turn)",
                false,
            );
            state.resolve_choose_one(opt0, opt1, pool, rng);
        }
        SPELL_TRICKY_TROUSERS => {
            if board_pos < state.board.len() {
                let (atk, hp) = state.auras.spell_stat_buff(1, 2);
                state.board[board_pos].add_stats(atk, hp);
                state.board[board_pos].taunt = !state.board[board_pos].taunt;
            }
        }
        SPELL_WEALTHY_BOUNTY => {
            let cap = 10 + state.auras.base_max_gold_bonus;
            state.gold = (state.gold + 2).min(cap);
        }
        SPELL_GEM_CONFISCATION => {
            if board_pos < state.board.len() {
                state.board[board_pos].play_blood_gems(3, &state.auras);
                let mut neighbor_indices = Vec::new();
                if board_pos > 0 {
                    neighbor_indices.push(board_pos - 1);
                }
                if board_pos + 1 < state.board.len() {
                    neighbor_indices.push(board_pos + 1);
                }
                let mut total_stolen_gems = 0u32;
                let mut total_stolen_atk = 0i32;
                let mut total_stolen_hp = 0i32;
                for n_idx in neighbor_indices {
                    let neighbor = &mut state.board[n_idx];
                    if neighbor.blood_gems_played > 0 {
                        let (s_atk, s_hp) = neighbor.blood_gem_stats_applied;
                        total_stolen_gems += neighbor.blood_gems_played;
                        total_stolen_atk += s_atk;
                        total_stolen_hp += s_hp;
                        neighbor.attack = (neighbor.attack - s_atk).max(0);
                        neighbor.health = (neighbor.health - s_hp).max(1);
                        neighbor.blood_gems_played = 0;
                        neighbor.blood_gem_stats_applied = (0, 0);
                    }
                }
                if total_stolen_gems > 0 {
                    let target = &mut state.board[board_pos];
                    target.blood_gems_played += total_stolen_gems;
                    target.blood_gem_stats_applied.0 += total_stolen_atk;
                    target.blood_gem_stats_applied.1 += total_stolen_hp;
                    target.add_stats(total_stolen_atk, total_stolen_hp);
                }
            }
        }
        SPELL_GOLDEN_TOUCH => {
            let candidates: Vec<usize> = state
                .shop
                .iter()
                .enumerate()
                .filter(|(_, u)| !u.is_spell && !u.is_golden)
                .map(|(i, _)| i)
                .collect();
            if !candidates.is_empty() {
                let pick = if candidates.len() == 1 {
                    candidates[0]
                } else {
                    candidates[rng.below(candidates.len())]
                };
                let target = &mut state.shop[pick];
                target.is_golden = true;
                target.add_stats(target.base_attack, target.base_health);
                target.base_attack *= 2;
                target.base_health *= 2;
            }
        }
        _ => {
            if board_pos < state.board.len() {
                state.board[board_pos].add_stats(card.attack, card.health);
            }
        }
    }
}
