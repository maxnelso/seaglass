//! Tier 1 through Tier 7 Tavern Spells (`Patch 36.6.3`).

use crate::cards::tokens::{
    self, is_choice_option, make_choice_option, CHOICE_ALLIANCE_ATK, CHOICE_ALLIANCE_HP,
    CHOICE_BOUNDLESS_MINION, CHOICE_BOUNDLESS_SPELL, CHOICE_FOREST_ALL, CHOICE_FOREST_SINGLE,
    CHOICE_GEM_DAY_ATK, CHOICE_GEM_DAY_HP, CHOICE_HP_1, CHOICE_HP_2, CHOICE_HP_3,
    CHOICE_TIME_MGMT_LATER, CHOICE_TIME_MGMT_NOW, SPELL_ARCANE_ABSORPTION, SPELL_BLOOD_GEM,
    SPELL_CONFLAGRATION, SPELL_GEM_CONFISCATION, SPELL_GEM_DAY, SPELL_GOLDEN_TOUCH,
    SPELL_POINTY_ARROW, SPELL_SLUDGE_CORROSION, SPELL_TAVERN_COIN,
};
use crate::cards::{board_passive, CardHooks, Passive};
use crate::model::{CardId, Keyword, Tribe, Unit, BONUS_KEYWORDS, SINGLE_TRIBES};
use crate::rng::Rng;
use crate::tavern::{shop_capacity, CardPool, TavernState};

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

// Tier 4 Tavern Spells (16 = 14 below + SPELL_GEM_CONFISCATION + SPELL_SLUDGE_CORROSION)
pub const SPELL_BLOOD_GEM_BARRAGE: CardId = 831;
pub const SPELL_BOON_OF_BEETLES: CardId = 832;
pub const SPELL_BOUNDLESS_POTENTIAL: CardId = 833;
pub const SPELL_CLONING_CONCH: CardId = 834;
pub const SPELL_DEFENDERS_RITES: CardId = 835;
pub const SPELL_EASTERLY_WINDS: CardId = 836;
pub const SPELL_EONARS_FAVOR: CardId = 837;
pub const SPELL_METHODICAL_MADNESS: CardId = 838;
pub const SPELL_MIGHTY_DRAGONBREATH: CardId = 839;
pub const SPELL_MISPLACED_TEA_SET: CardId = 840;
pub const SPELL_NATURAL_BLESSING: CardId = 841;
pub const SPELL_TEMPERATURE_SHIFT: CardId = 842;
pub const SPELL_TOMB_TURNING: CardId = 843;
pub const SPELL_WEAPONS_FORGE: CardId = 844;

// Tier 5 Tavern Spells (15 = 14 below + SPELL_GOLDEN_TOUCH)
pub const SPELL_ARMOR_STASH: CardId = 845;
pub const SPELL_BROOD_OF_NOZDORMU: CardId = 846;
pub const SPELL_BUTCHERING: CardId = 847;
pub const SPELL_CHANNEL_THE_DEVOURER: CardId = 848;
pub const SPELL_CONTRACTED_CORPSE: CardId = 849;
pub const SPELL_CORRUPTED_COIN: CardId = 850;
pub const SPELL_CORRUPTED_CUPCAKES: CardId = 851;
pub const SPELL_ENERGIZING_CHAMBER: CardId = 852;
pub const SPELL_FORESTS_BOUNTY: CardId = 853;
pub const SPELL_HIRED_HEADHUNTER: CardId = 854;
pub const SPELL_SALOONS_FINEST: CardId = 855;
pub const SPELL_UNMASKED_IDENTITY: CardId = 856;
pub const SPELL_UPPER_HAND: CardId = 857;
pub const SPELL_WAVE_OF_GOLD: CardId = 858;

// Tier 6 Tavern Spells (5)
pub const SPELL_AZERITE_EMPOWERMENT: CardId = 859;
pub const SPELL_EYES_OF_THE_EARTH_MOTHER: CardId = 860;
pub const SPELL_FANDRALS_FORTUNE: CardId = 861;
pub const SPELL_LOST_STAFF_OF_HAMUUL: CardId = 862;
pub const SPELL_PERFECT_VISION: CardId = 863;

// Tier 7 Tavern Spells (4)
pub const SPELL_HALLOWED_RITUAL: CardId = 864;
pub const SPELL_MENAGERIE_TABLEWARE: CardId = 865;
pub const SPELL_SACRED_GIFT: CardId = 866;
pub const SPELL_SHARING_IS_CARING: CardId = 867;

/// All 5 Bounty Tavern Spells (`Bigwig Bandit`, `Shipwrecked Rascal`, `Proud Privateer`).
pub const BOUNTY_SPELL_IDS: [CardId; 5] = [
    SPELL_FRIENDLY_BOUNTY,
    SPELL_HEALTHY_BOUNTY,
    SPELL_HOSTILE_BOUNTY,
    SPELL_SELFISH_BOUNTY,
    SPELL_WEALTHY_BOUNTY,
];

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

/// Construct a `Seafood Stew` spell card (`Gormling Gourmet`).
pub fn make_seafood_stew() -> Unit {
    make_tavern_spell(SPELL_SEAFOOD_STEW, "Seafood Stew", 3, 2, false)
}

/// Construct a `Blood Gem Barrage` spell card (`Razorfen Flapper`).
pub fn make_blood_gem_barrage() -> Unit {
    make_tavern_spell(SPELL_BLOOD_GEM_BARRAGE, "Blood Gem Barrage", 4, 1, false)
}

/// Construct a `Methodical Madness` spell card (`Imp-lusionist`).
pub fn make_methodical_madness() -> Unit {
    make_tavern_spell(SPELL_METHODICAL_MADNESS, "Methodical Madness", 4, 3, false)
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
        make_seafood_stew(),
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

/// All 16 active Tier 4 Tavern Spells (Patch 36.6.3).
pub fn tier4_spells() -> Vec<Unit> {
    vec![
        make_blood_gem_barrage(),
        make_tavern_spell(SPELL_BOON_OF_BEETLES, "Boon of Beetles", 4, 1, false),
        make_tavern_spell(
            SPELL_BOUNDLESS_POTENTIAL,
            "Boundless Potential",
            4,
            3,
            false,
        ),
        make_tavern_spell(SPELL_CLONING_CONCH, "Cloning Conch", 4, 4, false),
        make_tavern_spell(SPELL_DEFENDERS_RITES, "Defender's Rites", 4, 2, false),
        make_tavern_spell(SPELL_EASTERLY_WINDS, "Easterly Winds", 4, 1, false),
        make_tavern_spell(SPELL_EONARS_FAVOR, "Eonar's Favor", 4, 2, false),
        tokens::make_gem_confiscation(),
        make_methodical_madness(),
        make_tavern_spell(
            SPELL_MIGHTY_DRAGONBREATH,
            "Mighty Dragonbreath",
            4,
            2,
            false,
        ),
        make_tavern_spell(SPELL_MISPLACED_TEA_SET, "Misplaced Tea Set", 4, 3, false),
        make_tavern_spell(SPELL_NATURAL_BLESSING, "Natural Blessing", 4, 2, false),
        tokens::make_sludge_corrosion(),
        make_tavern_spell(SPELL_TEMPERATURE_SHIFT, "Temperature Shift", 4, 4, false),
        make_tavern_spell(SPELL_TOMB_TURNING, "Tomb Turning", 4, 2, false),
        make_tavern_spell(SPELL_WEAPONS_FORGE, "Weapons Forge", 4, 2, false),
    ]
}

/// All 15 active Tier 5 Tavern Spells (Patch 36.6.3).
pub fn tier5_spells() -> Vec<Unit> {
    vec![
        make_tavern_spell(SPELL_ARMOR_STASH, "Armor Stash", 5, 3, false),
        make_tavern_spell(SPELL_BROOD_OF_NOZDORMU, "Brood of Nozdormu", 5, 2, false),
        make_tavern_spell(SPELL_BUTCHERING, "Butchering", 5, 3, false),
        make_tavern_spell(
            SPELL_CHANNEL_THE_DEVOURER,
            "Channel the Devourer",
            5,
            4,
            false,
        ),
        make_tavern_spell(SPELL_CONTRACTED_CORPSE, "Contracted Corpse", 5, 3, false),
        make_tavern_spell(SPELL_CORRUPTED_COIN, "Corrupted Coin", 5, 2, false),
        make_tavern_spell(SPELL_CORRUPTED_CUPCAKES, "Corrupted Cupcakes", 5, 4, false),
        make_tavern_spell(SPELL_ENERGIZING_CHAMBER, "Energizing Chamber", 5, 1, false),
        make_tavern_spell(SPELL_FORESTS_BOUNTY, "Forest's Bounty", 5, 2, false),
        tokens::make_golden_touch(),
        make_tavern_spell(SPELL_HIRED_HEADHUNTER, "Hired Headhunter", 5, 3, false),
        make_tavern_spell(SPELL_SALOONS_FINEST, "Saloon's Finest", 5, 2, false),
        make_tavern_spell(SPELL_UNMASKED_IDENTITY, "Unmasked Identity", 5, 3, false),
        make_tavern_spell(SPELL_UPPER_HAND, "Upper Hand", 5, 3, false),
        make_tavern_spell(SPELL_WAVE_OF_GOLD, "Wave of Gold", 5, 2, false),
    ]
}

/// All 5 active Tier 6 Tavern Spells (Patch 36.6.3).
pub fn tier6_spells() -> Vec<Unit> {
    vec![
        make_tavern_spell(SPELL_AZERITE_EMPOWERMENT, "Azerite Empowerment", 6, 4, false),
        make_tavern_spell(
            SPELL_EYES_OF_THE_EARTH_MOTHER,
            "Eyes of the Earth Mother",
            6,
            4,
            false,
        ),
        make_tavern_spell(SPELL_FANDRALS_FORTUNE, "Fandral's Fortune", 6, 3, false),
        make_tavern_spell(
            SPELL_LOST_STAFF_OF_HAMUUL,
            "Lost Staff of Hamuul",
            6,
            2,
            false,
        ),
        make_tavern_spell(SPELL_PERFECT_VISION, "Perfect Vision", 6, 2, false),
    ]
}

/// All 4 active Tier 7 Tavern Spells (Patch 36.6.3).
pub fn tier7_spells() -> Vec<Unit> {
    vec![
        make_tavern_spell(SPELL_HALLOWED_RITUAL, "Hallowed Ritual", 7, 5, false),
        make_tavern_spell(
            SPELL_MENAGERIE_TABLEWARE,
            "Menagerie Tableware",
            7,
            4,
            false,
        ),
        make_tavern_spell(SPELL_SACRED_GIFT, "Sacred Gift", 7, 4, false),
        make_tavern_spell(SPELL_SHARING_IS_CARING, "Sharing is Caring", 7, 2, false),
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
    if max_tier >= 4 {
        list.extend(tier4_spells());
    }
    if max_tier >= 5 {
        list.extend(tier5_spells());
    }
    if max_tier >= 6 {
        list.extend(tier6_spells());
    }
    if max_tier >= 7 {
        list.extend(tier7_spells());
    }
    list
}

/// Look up a Tavern Spell or token spell by `CardId`.
pub fn spell_by_id(card_id: CardId) -> Option<Unit> {
    match card_id {
        SPELL_BLOOD_GEM => return Some(tokens::make_blood_gem()),
        tokens::SPELL_LOCKBOX => return Some(tokens::make_lockbox()),
        SPELL_GEM_DAY => return Some(tokens::make_gem_day()),
        SPELL_SLUDGE_CORROSION => return Some(tokens::make_sludge_corrosion()),
        SPELL_GEM_CONFISCATION => return Some(tokens::make_gem_confiscation()),
        SPELL_GOLDEN_TOUCH => return Some(tokens::make_golden_touch()),
        SPELL_POINTY_ARROW => return Some(tokens::make_pointy_arrow()),
        SPELL_ARCANE_ABSORPTION => return Some(tokens::make_arcane_absorption()),
        SPELL_CONFLAGRATION => return Some(tokens::make_conflagration()),
        _ => {}
    }
    spells_up_to_tier(7).into_iter().find(|s| s.card_id == card_id)
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
        "Pointy Arrow" => return Some(tokens::make_pointy_arrow()),
        "Arcane Absorption" => return Some(tokens::make_arcane_absorption()),
        "Conflagration" => return Some(tokens::make_conflagration()),
        _ => {}
    }
    spells_up_to_tier(7).into_iter().find(|s| s.name == name)
}

/// Returns `true` if `card_id` is a collectible Tavern spell eligible for the shop and random/Discover Tavern spell pools
/// (excludes minion-generated token spells and unimplemented Hero Power stubs).
pub fn is_pool_tavern_spell(card_id: CardId) -> bool {
    !matches!(
        card_id,
        SPELL_GEM_CONFISCATION
            | SPELL_SLUDGE_CORROSION
            | SPELL_GOLDEN_TOUCH
            | SPELL_UNMASKED_IDENTITY
    )
}

/// Draw a uniformly random Tavern Spell with `tavern_tier <= max_tier`.
pub fn draw_random_tavern_spell(max_tier: u32, rng: &mut Rng) -> Unit {
    let pool: Vec<Unit> = spells_up_to_tier(max_tier)
        .into_iter()
        .filter(|s| is_pool_tavern_spell(s.card_id))
        .collect();
    let idx = rng.below(pool.len());
    pool[idx].clone()
}

/// Draw a uniformly random Bounty spell (`Bigwig Bandit`).
pub fn draw_random_bounty(rng: &mut Rng) -> Unit {
    let cid = BOUNTY_SPELL_IDS[rng.below(BOUNTY_SPELL_IDS.len())];
    tier3_spells()
        .into_iter()
        .find(|s| s.card_id == cid)
        .unwrap_or_else(|| make_tavern_spell(SPELL_FRIENDLY_BOUNTY, "Friendly Bounty", 3, 2, false))
}

/// Draw a uniformly random Tavern Spell with exact `cost` (e.g. 1-Cost for `Unwilling Slacker` / `Gearfin`, 2-Cost for `Blue Chromadrake`).
pub fn draw_random_cost_tavern_spell(cost: u32, rng: &mut Rng) -> Unit {
    let pool: Vec<Unit> = spells_up_to_tier(6)
        .into_iter()
        .filter(|s| is_pool_tavern_spell(s.card_id) && s.spell_cost == cost && !s.costs_health)
        .collect();
    let idx = rng.below(pool.len());
    pool[idx].clone()
}

/// Draw up to `count` distinct Tavern Spells with `tavern_tier <= max_tier` for Discover.
pub fn draw_discover_tavern_spells(max_tier: u32, count: usize, rng: &mut Rng) -> Vec<Unit> {
    let mut pool: Vec<Unit> = spells_up_to_tier(max_tier)
        .into_iter()
        .filter(|s| is_pool_tavern_spell(s.card_id))
        .collect();
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

/// Draw up to `count` distinct Tavern Spells of exact `tier` (falling back to `<= tier` if none at `tier`) for `Boundless Potential`.
pub fn draw_discover_tavern_spells_exact_tier(tier: u32, count: usize, rng: &mut Rng) -> Vec<Unit> {
    let mut pool: Vec<Unit> = spells_up_to_tier(tier)
        .into_iter()
        .filter(|s| is_pool_tavern_spell(s.card_id) && s.tavern_tier == tier)
        .collect();
    if pool.is_empty() {
        pool = spells_up_to_tier(tier)
            .into_iter()
            .filter(|s| is_pool_tavern_spell(s.card_id))
            .collect();
    }
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

/// Determine the player's most common minion tribe on `board` (`Friendly Bounty`, `Planar Telescope`, `Dark Paradox`).
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
            | SPELL_DEFENDERS_RITES
            | SPELL_EONARS_FAVOR
            | SPELL_METHODICAL_MADNESS
            | SPELL_NATURAL_BLESSING
            | SPELL_POINTY_ARROW
            | SPELL_ARCANE_ABSORPTION
            | SPELL_CONFLAGRATION
            | SPELL_BUTCHERING
            | SPELL_CHANNEL_THE_DEVOURER
            | SPELL_CORRUPTED_CUPCAKES
            | SPELL_FORESTS_BOUNTY
            | SPELL_EYES_OF_THE_EARTH_MOTHER
            | SPELL_LOST_STAFF_OF_HAMUUL
            | SPELL_PERFECT_VISION
            | SPELL_SACRED_GIFT
    )
}

/// Check whether `unit` is a valid board target for targeted spell `spell_id`.
pub fn can_target(spell_id: CardId, unit: &Unit) -> bool {
    match spell_id {
        SPELL_BUTCHERING => unit.tribe.matches(Tribe::Undead),
        SPELL_CORRUPTED_CUPCAKES => unit.tribe.matches(Tribe::Demon),
        SPELL_ARCANE_ABSORPTION => unit.tribe.matches(Tribe::Elemental),
        _ => true,
    }
}

/// Select up to one distinct living friendly minion on `board` for each single tribe (`Misplaced Tea Set`, `Veteran Technican`, `The Last One Standing`).
pub(crate) fn select_menagerie_targets(board: &[Unit], rng: &mut Rng) -> Vec<usize> {
    let mut chosen_indices: Vec<usize> = Vec::new();
    for &tribe in &SINGLE_TRIBES {
        let candidates: Vec<usize> = board
            .iter()
            .enumerate()
            .filter(|(i, u)| u.health > 0 && !chosen_indices.contains(i) && u.tribe.matches(tribe))
            .map(|(i, _)| i)
            .collect();
        if !candidates.is_empty() {
            let pick = if candidates.len() == 1 {
                candidates[0]
            } else {
                candidates[rng.below(candidates.len())]
            };
            chosen_indices.push(pick);
        }
    }
    chosen_indices
}

/// Apply the `Misplaced Tea Set` effect (`Give a friendly minion of each type +4/+4`).
pub fn apply_misplaced_tea_set(state: &mut TavernState, rng: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(4, 4);
    let chosen_indices = select_menagerie_targets(&state.board, rng);
    for idx in chosen_indices {
        state.board[idx].add_stats(atk, hp);
    }
}

/// Draw up to `count` distinct Choose-One cards (minions and spells) with both effects combined (`Fandral's Fortune`).
pub fn draw_discover_choose_one(state: &TavernState, count: usize, rng: &mut Rng) -> Vec<Unit> {
    let max_tier = state.tavern_tier.max(1);
    let mut all_choose_one: Vec<Unit> = crate::cards::full_catalog()
        .into_iter()
        .filter(|t| crate::cards::is_choose_one_minion(t.card_id))
        .map(|t| {
            let mut u = t.instantiate();
            state.apply_global_unit_auras(&mut u);
            u
        })
        .collect();
    for spell_id in [
        SPELL_ALLIANCE_FLAG,
        SPELL_GEM_DAY,
        SPELL_TIME_MANAGEMENT,
        SPELL_BOUNDLESS_POTENTIAL,
        SPELL_FORESTS_BOUNTY,
    ] {
        if let Some(s) = spell_by_id(spell_id) {
            all_choose_one.push(s);
        }
    }
    let mut pool: Vec<Unit> = all_choose_one
        .iter()
        .filter(|u| u.tavern_tier <= max_tier)
        .cloned()
        .collect();
    if pool.len() < count {
        pool = all_choose_one;
    }
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
        let mut opt = pool.remove(idx);
        opt.fandral_combined = true;
        out.push(opt);
    }
    out
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
    let targeted = spell_requires_board_target(card.card_id);
    if is_tavern_spell(card.card_id) {
        state.auras.last_tavern_spell_cast = Some(card.card_id);
        crate::cards::on_cast_tavern_spell(state, pool, rng);
    }

    let mut casts = if BOUNTY_SPELL_IDS.contains(&card.card_id) {
        board_passive(&state.board, Passive::BountyCasts)
    } else {
        1
    };
    if targeted {
        casts *= board_passive(&state.board, Passive::TargetedSpellCasts);
    }

    for _ in 0..casts {
        state.fandral_combined_active = card.fandral_combined;
        match card.card_id {
            SPELL_BLOOD_GEM => {
                if board_pos < state.board.len() {
                    let extra = board_passive(&state.board, Passive::ExtraHandBloodGemCasts);
                    state.board[board_pos].play_blood_gems(1 + extra, &state.auras);
                    crate::cards::resolve_pending_effects(&mut state.board, &state.auras, rng);
                }
            }
            SPELL_POINTY_ARROW => {
                if board_pos < state.board.len() {
                    let (atk, hp) = state.auras.spell_stat_buff(4, 0);
                    state.board[board_pos].add_stats(atk, hp);
                }
            }
            SPELL_ARCANE_ABSORPTION => {
                if board_pos < state.board.len()
                    && state.board[board_pos].tribe.matches(Tribe::Elemental)
                {
                    let max_hp = state
                        .shop
                        .iter()
                        .filter(|u| !u.is_spell)
                        .map(|u| u.health)
                        .max();
                    if let Some(best_hp) = max_hp {
                        let candidates: Vec<(i32, i32)> = state
                            .shop
                            .iter()
                            .filter(|u| !u.is_spell && u.health == best_hp)
                            .map(|u| (u.attack / 2, u.health / 2))
                            .collect();
                        let (base_atk, base_hp) = if candidates.len() == 1 {
                            candidates[0]
                        } else {
                            candidates[rng.below(candidates.len())]
                        };
                        let (atk, hp) = state.auras.spell_stat_buff(base_atk, base_hp);
                        state.board[board_pos].add_stats(atk, hp);
                    }
                }
            }
            SPELL_CONFLAGRATION => {
                if board_pos < state.board.len() {
                    let base = 4 + state.elementals_played_this_turn as i32;
                    let (atk, hp) = state.auras.spell_stat_buff(base, base);
                    state.board[board_pos].add_stats(atk, hp);
                }
            }
            SPELL_TAVERN_COIN | SPELL_HASTY_EXCAVATION => {
                state.gold += 1;
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
                    let opt0 =
                        make_choice_option(CHOICE_ALLIANCE_ATK, "Alliance Flag (+3/+1)", false);
                    let opt1 =
                        make_choice_option(CHOICE_ALLIANCE_HP, "Alliance Flag (+1/+3)", false);
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
                    if let Some(mut drawn) =
                        pool.draw_by_tribe(tribe, None, state.tavern_tier, rng)
                    {
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
                        state.sync_all_auras();
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
                state.gold += 2;
            }
            SPELL_BLOOD_GEM_BARRAGE => {
                state.auras.blood_gem_barrage_stacks += 1;
            }
            SPELL_BOON_OF_BEETLES => {
                state.auras.boon_of_beetles_charges += 2;
            }
            SPELL_BOUNDLESS_POTENTIAL => {
                let opt0 = make_choice_option(
                    CHOICE_BOUNDLESS_MINION,
                    "Way of the Warrior (Discover a minion of your Tier)",
                    false,
                );
                let opt1 = make_choice_option(
                    CHOICE_BOUNDLESS_SPELL,
                    "Way of the Mage (Discover a Tavern spell of your Tier)",
                    false,
                );
                state.resolve_choose_one(opt0, opt1, pool, rng);
            }
            SPELL_CLONING_CONCH => {
                if let Some(mut drawn) =
                    pool.draw_by_tribe(Tribe::Murloc, None, state.tavern_tier, rng)
                {
                    state.apply_global_unit_auras(&mut drawn);
                    let copy = drawn.clone();
                    state.add_to_hand(drawn);
                    state.add_to_hand(copy);
                }
            }
            SPELL_DEFENDERS_RITES => {
                if board_pos < state.board.len() {
                    let (atk, hp) = state.auras.spell_stat_buff(7, 7);
                    state.board[board_pos].add_stats(atk, hp);
                    state.board[board_pos].apply_keyword(Keyword::Taunt, false);
                }
            }
            SPELL_EASTERLY_WINDS => {
                let (atk, hp) = state.auras.spell_stat_buff(9, 9);
                state.auras.refresh_random_buffs.push((atk, hp));
            }
            SPELL_EONARS_FAVOR => {
                if board_pos < state.board.len() {
                    let target_tribe = state.board[board_pos].tribe;
                    if target_tribe != Tribe::None {
                        let (atk, hp) = state.auras.spell_stat_buff(3, 3);
                        state.auras.tavern_tribe_buffs.push((target_tribe, atk, hp));
                        for u in &mut state.shop {
                            if !u.is_spell && u.tribe.matches(target_tribe) {
                                u.add_stats(atk, hp);
                            }
                        }
                    }
                }
            }
            SPELL_METHODICAL_MADNESS => {
                if board_pos < state.board.len() {
                    for _ in 0..2 {
                        let shop_minions: Vec<usize> = state
                            .shop
                            .iter()
                            .enumerate()
                            .filter(|(_, u)| !u.is_spell)
                            .map(|(i, _)| i)
                            .collect();
                        if shop_minions.is_empty() {
                            break;
                        }
                        let pick = if shop_minions.len() == 1 {
                            shop_minions[0]
                        } else {
                            shop_minions[rng.below(shop_minions.len())]
                        };
                        let consumed = state.shop.remove(pick);
                        pool.return_unit(&consumed);
                        let target = &mut state.board[board_pos];
                        target.add_stats(consumed.attack, consumed.health);
                        for kw in BONUS_KEYWORDS {
                            if consumed.has_keyword(kw) {
                                target.apply_keyword(kw, false);
                            }
                        }
                    }
                }
            }
            SPELL_MIGHTY_DRAGONBREATH => {
                let (atk, hp) = state.auras.spell_stat_buff(3, 2);
                for u in &mut state.board {
                    u.add_stats(atk, hp);
                    if u.tribe.matches(Tribe::Dragon) {
                        u.add_stats(atk, hp);
                    }
                    if u.divine_shield {
                        u.add_stats(atk, hp);
                    }
                }
            }
            SPELL_MISPLACED_TEA_SET => {
                apply_misplaced_tea_set(state, rng);
            }
            SPELL_NATURAL_BLESSING => {
                if board_pos < state.board.len() {
                    let target_tribe = state.board[board_pos].tribe;
                    if target_tribe != Tribe::None {
                        let (atk, hp) = state.auras.spell_stat_buff(2, 1);
                        for u in &mut state.board {
                            if u.tribe.matches(target_tribe) {
                                u.add_stats(atk, hp);
                            }
                        }
                        for u in &mut state.shop {
                            if !u.is_spell && u.tribe.matches(target_tribe) {
                                u.add_stats(atk, hp);
                            }
                        }
                    }
                }
            }
            SPELL_TEMPERATURE_SHIFT => {
                let mut fire = crate::cards::tier2::fire_baller::template().instantiate();
                state.apply_global_unit_auras(&mut fire);
                state.add_to_hand(fire);
                let mut snow = crate::cards::tier2::snow_baller::template().instantiate();
                state.apply_global_unit_auras(&mut snow);
                state.add_to_hand(snow);
            }
            SPELL_TOMB_TURNING => {
                let mut opts =
                    pool.draw_discover_by_tribe(Tribe::Undead, state.tavern_tier, 3, rng);
                for opt in &mut opts {
                    state.apply_global_unit_auras(opt);
                    opt.dies_on_play_this_turn = true;
                }
                if !opts.is_empty() {
                    state.push_discover(opts);
                }
            }
            SPELL_WEAPONS_FORGE => {
                for _ in 0..3 {
                    state.add_to_hand(tokens::make_pointy_arrow());
                }
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
                    crate::cards::resolve_pending_effects(&mut state.board, &state.auras, rng);
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
                    state.shop[pick].make_golden();
                    crate::cards::sync_unit_auras(&mut state.shop[pick], &state.auras);
                }
            }
            SPELL_ARMOR_STASH => {
                state.armor = 5;
            }
            SPELL_BROOD_OF_NOZDORMU => {
                state.auras.brood_of_nozdormu_stacks += 1;
            }
            SPELL_BUTCHERING => {
                if board_pos < state.board.len()
                    && state.board[board_pos].tribe.matches(Tribe::Undead)
                {
                    state.destroy_board_unit(board_pos, pool, rng);
                    let (atk, _hp) = state.auras.spell_stat_buff(8, 0);
                    state.auras.undead_bonus_attack += atk;
                    state.sync_all_auras();
                }
            }
            SPELL_CHANNEL_THE_DEVOURER => {
                if board_pos < state.board.len() {
                    let sold = state.board.remove(board_pos);
                    let (sold_atk, sold_hp) = (sold.attack, sold.health);
                    pool.return_unit(&sold);
                    state.gold += 1;
                    crate::cards::on_sell(state, &sold, pool, rng);
                    if !state.board.is_empty() {
                        let pick = if state.board.len() == 1 {
                            0
                        } else {
                            rng.below(state.board.len())
                        };
                        let (atk, hp) = state.auras.spell_stat_buff(sold_atk, sold_hp);
                        state.board[pick].add_stats(atk, hp);
                    }
                }
            }
            SPELL_CONTRACTED_CORPSE => {
                let mut opts = pool.draw_discover_filtered(
                    state.tavern_tier,
                    3,
                    crate::cards::is_deathrattle_minion,
                    rng,
                );
                for opt in &mut opts {
                    state.apply_global_unit_auras(opt);
                }
                if !opts.is_empty() {
                    state.push_discover(opts);
                }
            }
            SPELL_CORRUPTED_COIN => {
                state.gold += 2;
            }
            SPELL_CORRUPTED_CUPCAKES => {
                if board_pos < state.board.len()
                    && state.board[board_pos].tribe.matches(Tribe::Demon)
                {
                    for _ in 0..3 {
                        let shop_minions: Vec<usize> = state
                            .shop
                            .iter()
                            .enumerate()
                            .filter(|(_, u)| !u.is_spell)
                            .map(|(i, _)| i)
                            .collect();
                        if shop_minions.is_empty() {
                            break;
                        }
                        let pick = if shop_minions.len() == 1 {
                            shop_minions[0]
                        } else {
                            shop_minions[rng.below(shop_minions.len())]
                        };
                        let consumed = state.shop.remove(pick);
                        pool.return_unit(&consumed);
                        state.board[board_pos].add_stats(consumed.attack, consumed.health);
                    }
                }
            }
            SPELL_ENERGIZING_CHAMBER => {
                let (atk, hp) = state.auras.spell_stat_buff(7, 7);
                state.auras.deity.attack += atk;
                state.auras.deity.health += hp;
            }
            SPELL_FORESTS_BOUNTY => {
                if board_pos < state.board.len() {
                    state.pending_choice_target = Some(board_pos);
                    let opt0 = make_choice_option(
                        CHOICE_FOREST_SINGLE,
                        "Forest's Bounty (+6/+6 twice)",
                        false,
                    );
                    let opt1 = make_choice_option(
                        CHOICE_FOREST_ALL,
                        "Forest's Bounty (+2/+2 to all)",
                        false,
                    );
                    state.resolve_choose_one(opt0, opt1, pool, rng);
                }
            }
            SPELL_HIRED_HEADHUNTER => {
                let mut opts = pool.draw_discover_filtered(
                    state.tavern_tier,
                    3,
                    crate::cards::is_battlecry_minion,
                    rng,
                );
                for opt in &mut opts {
                    state.apply_global_unit_auras(opt);
                }
                if !opts.is_empty() {
                    state.push_discover(opts);
                }
            }
            SPELL_SALOONS_FINEST => {
                state.is_frozen = false;
                for old in state.shop.drain(..) {
                    pool.return_unit(&old);
                }
                let cap = shop_capacity(state.tavern_tier);
                for _ in 0..cap {
                    state
                        .shop
                        .push(draw_random_tavern_spell(state.tavern_tier, rng));
                }
            }
            SPELL_UNMASKED_IDENTITY => {
                let opts = vec![
                    make_choice_option(CHOICE_HP_1, "Hero Power Option 1", false),
                    make_choice_option(CHOICE_HP_2, "Hero Power Option 2", false),
                    make_choice_option(CHOICE_HP_3, "Hero Power Option 3", false),
                ];
                state.push_discover(opts);
            }
            SPELL_UPPER_HAND => {
                state.auras.upper_hand_stacks += 1;
            }
            SPELL_WAVE_OF_GOLD => {
                let (atk, hp) = state.auras.spell_stat_buff(3, 2);
                for u in &mut state.board {
                    u.add_stats(atk, hp);
                    if u.is_golden {
                        u.add_stats(atk, hp);
                    }
                }
            }
            SPELL_AZERITE_EMPOWERMENT => {
                let (atk, hp) = state.auras.spell_stat_buff(2, 2);
                for _ in 0..2 {
                    for u in &mut state.board {
                        u.add_stats(atk, hp);
                    }
                }
            }
            SPELL_EYES_OF_THE_EARTH_MOTHER => {
                if board_pos < state.board.len()
                    && state.board[board_pos].tavern_tier <= 4
                    && !state.board[board_pos].is_golden
                {
                    state.board[board_pos].make_golden();
                    state.sync_all_auras();
                }
            }
            SPELL_FANDRALS_FORTUNE => {
                let opts = draw_discover_choose_one(state, 3, rng);
                if !opts.is_empty() {
                    state.push_discover(opts);
                }
            }
            SPELL_LOST_STAFF_OF_HAMUUL => {
                if board_pos < state.board.len() {
                    let target_tribe = state.board[board_pos].tribe;
                    state.refresh_shop_with_tribe(target_tribe, pool, rng);
                }
            }
            SPELL_PERFECT_VISION => {
                if board_pos < state.board.len() {
                    let (atk, hp) = state.auras.spell_stat_buff(20, 20);
                    let target = &mut state.board[board_pos];
                    target.attack = atk;
                    target.health = hp;
                    target.sync_max_stats();
                    crate::cards::check_stat_thresholds(target);
                }
            }
            SPELL_HALLOWED_RITUAL => {
                let mut opts = pool.draw_discover_options(7, 3, rng);
                for opt in &mut opts {
                    state.apply_global_unit_auras(opt);
                }
                if !opts.is_empty() {
                    state.push_discover(opts);
                }
            }
            SPELL_MENAGERIE_TABLEWARE => {
                let repeats = 1 + select_menagerie_targets(&state.board, rng).len();
                let (atk, hp) = state.auras.spell_stat_buff(3, 3);
                for _ in 0..repeats {
                    for u in &mut state.board {
                        u.add_stats(atk, hp);
                    }
                }
            }
            SPELL_SACRED_GIFT => {
                if board_pos < state.board.len() {
                    state.board[board_pos].apply_keyword(Keyword::DivineShield, false);
                }
            }
            SPELL_SHARING_IS_CARING => {
                state.auras.sharing_is_caring_stacks += 1;
            }
            _ => {
                if board_pos < state.board.len() {
                    state.board[board_pos].add_stats(card.attack, card.health);
                }
            }
        }
        state.fandral_combined_active = false;
    }

    if targeted && board_pos < state.board.len() {
        crate::cards::after_cast_targeted_spell(state, board_pos, pool, rng);
    }

    crate::cards::after_cast_any_spell(state, pool, rng);
}

/// `Corrupted Coin` discarded: gain 2 maximum Gold.
fn corrupted_coin_discarded(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    state.auras.base_max_gold_bonus += 2;
    state.max_gold += 2;
}

/// `Energizing Chamber` discarded: cast it twice.
fn energizing_chamber_discarded(
    state: &mut TavernState,
    _: &Unit,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if let Some(chamber) = spell_by_id(SPELL_ENERGIZING_CHAMBER) {
        for _ in 0..2 {
            state.auras.spells_played += 1;
            cast_spell(state, chamber.clone(), 0, pool, rng);
        }
    }
}

// --- Choose-One options offered by Tavern spells (registered in `tokens::behaviors`) ---

/// `Gem Day`: your Blood Gems give an extra +1 Attack this game.
pub fn choose_gem_day_atk(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    state.auras.blood_gem_bonus_atk += 1;
}

/// `Gem Day`: your Blood Gems give an extra +1 Health this game.
pub fn choose_gem_day_hp(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    state.auras.blood_gem_bonus_hp += 1;
}

/// `Alliance Flag`: give the target +3/+1.
pub fn choose_alliance_atk(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(pos) = state.choice_target() {
        let (atk, hp) = state.auras.spell_stat_buff(3, 1);
        state.board[pos].add_stats(atk, hp);
    }
}

/// `Alliance Flag`: give the target +1/+3.
pub fn choose_alliance_hp(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(pos) = state.choice_target() {
        let (atk, hp) = state.auras.spell_stat_buff(1, 3);
        state.board[pos].add_stats(atk, hp);
    }
}

/// `Time Management` (Hurry Up): give your minions (board and hand) +2/+2 now.
pub fn choose_time_now(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(2, 2);
    for b in &mut state.board {
        b.add_stats(atk, hp);
    }
    for h in &mut state.hand {
        if !h.is_spell {
            h.add_stats(atk, hp);
        }
    }
}

/// `Time Management` (Do It Later): +2/+2 twice next turn.
pub fn choose_time_later(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    state.auras.time_management_next_turn += 2;
}

/// `Boundless Potential` (Way of the Warrior): Discover a minion of your Tier.
pub fn choose_boundless_minion(
    state: &mut TavernState,
    _: &Unit,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let mut opts = pool.draw_discover_options(state.tavern_tier, 3, rng);
    for opt in &mut opts {
        state.apply_global_unit_auras(opt);
    }
    if !opts.is_empty() {
        state.push_discover(opts);
    }
}

/// `Boundless Potential` (Way of the Mage): Discover a Tavern spell of your Tier.
pub fn choose_boundless_spell(state: &mut TavernState, _: &Unit, _: &mut CardPool, rng: &mut Rng) {
    let opts = draw_discover_tavern_spells_exact_tier(state.tavern_tier, 3, rng);
    if !opts.is_empty() {
        state.push_discover(opts);
    }
}

/// `Forest's Bounty`: give the target +6/+6 twice.
pub fn choose_forest_single(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(pos) = state.choice_target() {
        let (atk, hp) = state.auras.spell_stat_buff(6, 6);
        for _ in 0..2 {
            state.board[pos].add_stats(atk, hp);
        }
    }
}

/// `Forest's Bounty`: give your minions +2/+2.
pub fn choose_forest_all(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(2, 2);
    for b in &mut state.board {
        b.add_stats(atk, hp);
    }
}

/// `Unmasked Identity`: take the chosen Hero Power.
pub fn choose_hero_power(state: &mut TavernState, chosen: &Unit, _: &mut CardPool, _: &mut Rng) {
    state.auras.hero_power_id = match chosen.card_id {
        CHOICE_HP_1 => 1,
        CHOICE_HP_2 => 2,
        CHOICE_HP_3 => 3,
        _ => return,
    };
}

/// Behaviour tables for spells (registered in the card registry).
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            SPELL_CORRUPTED_COIN,
            CardHooks::EMPTY.on_discarded(corrupted_coin_discarded),
        ),
        (
            SPELL_ENERGIZING_CHAMBER,
            CardHooks::EMPTY.on_discarded(energizing_chamber_discarded),
        ),
    ]
}
