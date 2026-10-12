//! Tier 1 through Tier 7 Tavern Spells (`Patch 36.6.3`).
//!
//! Each spell (Tavern spells and token spells) lives in its own submodule with its `cast` hook
//! and other card text; this module holds the spell ids and tables, lookups and random draws,
//! the shared casting flow ([`cast_spell`]), and the registry table ([`behaviors`]).

pub mod a_new_sprout;
pub mod alliance_flag;
pub mod arcane_absorption;
pub mod armor_stash;
pub mod azerite_empowerment;
pub mod blood_gem;
pub mod blood_gem_barrage;
pub mod boon_of_beetles;
pub mod boundless_potential;
pub mod brood_of_nozdormu;
pub mod butchering;
pub mod careful_investment;
pub mod channel_the_devourer;
pub mod chefs_choice;
pub mod cloning_conch;
pub mod conflagration;
pub mod contracted_corpse;
pub mod corrupted_coin;
pub mod corrupted_cupcakes;
pub mod defenders_rites;
pub mod easterly_winds;
pub mod enchanted_lasso;
pub mod energizing_chamber;
pub mod eonars_favor;
pub mod eyes_of_the_earth_mother;
pub mod fandrals_fortune;
pub mod forests_bounty;
pub mod fortify;
pub mod friendly_bounty;
pub mod gem_confiscation;
pub mod gem_day;
pub mod golden_touch;
pub mod hallowed_ritual;
pub mod hasty_excavation;
pub mod healthy_bounty;
pub mod hired_headhunter;
pub mod hostile_bounty;
pub mod leaf_through_the_pages;
pub mod lockbox;
pub mod lost_staff_of_hamuul;
pub mod menagerie_tableware;
pub mod methodical_madness;
pub mod might_of_stormwind;
pub mod mighty_dragonbreath;
pub mod misplaced_tea_set;
pub mod natural_blessing;
pub mod overconfidence;
pub mod perfect_vision;
pub mod planar_telescope;
pub mod pointy_arrow;
pub mod recruit_a_trainee;
pub mod repair_job;
pub mod robust_evolution;
pub mod sacred_gift;
pub mod saloons_finest;
pub mod seafood_stew;
pub mod search_through_time;
pub mod selfish_bounty;
pub mod sharing_is_caring;
pub mod shiny_ring;
pub mod sludge_corrosion;
pub mod staff_of_enrichment;
pub mod strike_oil;
pub mod tavern_coin;
pub mod tavern_dish_banana;
pub mod temperature_shift;
pub mod them_apples;
pub mod time_management;
pub mod tomb_turning;
pub mod tricky_trousers;
pub mod unmasked_identity;
pub mod upper_hand;
pub mod wave_of_gold;
pub mod wealthy_bounty;
pub mod weapons_forge;
pub mod winners_bread;

use crate::cards::hooks::{CastAtFn, CastFn, SpellTarget};
use crate::cards::tokens::{self, is_choice_option, SPELL_GEM_DAY};
use crate::cards::{board_passive, hooks, CardFlags, CardHooks, Passive};
use crate::model::{CardId, EffectDuration, Tribe, Unit, UnitId, SINGLE_TRIBES};
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

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
    !hooks(card_id).has(CardFlags::NOT_TAVERN_SPELL) && !is_choice_option(card_id)
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

/// Every spell card: the Tier 1-7 Tavern Spells, then the token spells not among them.
pub fn all_spells() -> Vec<Unit> {
    let mut list = spells_up_to_tier(7);
    list.extend([
        tokens::make_blood_gem(),
        tokens::make_lockbox(),
        tokens::make_gem_day(),
        tokens::make_pointy_arrow(),
        tokens::make_arcane_absorption(),
        tokens::make_conflagration(),
    ]);
    list
}

/// Look up a Tavern Spell or token spell by `CardId`.
pub fn spell_by_id(card_id: CardId) -> Option<Unit> {
    all_spells().into_iter().find(|s| s.card_id == card_id)
}

/// Look up a Tavern Spell or token spell by exact name.
pub fn spell_by_name(name: &str) -> Option<Unit> {
    all_spells().into_iter().find(|s| s.name == name)
}

/// Returns `true` if `card_id` is a collectible Tavern spell eligible for the shop and random/Discover Tavern spell pools
/// (excludes [`CardFlags::NOT_IN_POOL`] ones: minion-generated token spells and unimplemented Hero
/// Power stubs).
pub fn is_pool_tavern_spell(card_id: CardId) -> bool {
    !hooks(card_id).has(CardFlags::NOT_IN_POOL)
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

/// Returns `true` if casting `card_id` from hand requires a target: a friendly minion on `board`
/// (or, for a [`SpellTarget::Minion`] spell, a minion in the Tavern; see [`can_target_in_shop`]).
pub fn spell_requires_board_target(card_id: CardId) -> bool {
    hooks(card_id).spell_target != SpellTarget::None
}

/// Check whether `unit` is a valid board target for targeted spell `spell_id`.
pub fn can_target(spell_id: CardId, unit: &Unit) -> bool {
    match hooks(spell_id).spell_target {
        SpellTarget::FriendlyTribe(tribe) => unit.tribe.matches(tribe),
        SpellTarget::FriendlyTyped => unit.tribe != Tribe::None,
        SpellTarget::None | SpellTarget::Friendly | SpellTarget::Minion => true,
    }
}

/// Check whether `unit`, in the Tavern, is a valid target for spell `spell_id`: a minion, for a
/// spell that can target any minion ([`SpellTarget::Minion`]).
pub fn can_target_in_shop(spell_id: CardId, unit: &Unit) -> bool {
    hooks(spell_id).spell_target == SpellTarget::Minion && !unit.is_spell
}

/// Each minion's types on `board`, as bits over [`SINGLE_TRIBES`] (none for a dead minion).
fn type_bits(board: &[Unit]) -> Vec<u16> {
    board
        .iter()
        .map(|u| {
            SINGLE_TRIBES
                .iter()
                .enumerate()
                .filter(|&(_, &tribe)| u.health > 0 && u.tribe.has_single(tribe))
                .fold(0, |bits, (t, _)| bits | 1 << t)
        })
        .collect()
}

/// Give minion `i` one of its `types`, moving the minion holding that type (`holder`, by type) to
/// another of its types if needed (Kuhn's augmenting path). `tried` holds the types already tried.
fn give_type(i: usize, types: &[u16], holder: &mut [Option<usize>], tried: &mut u16) -> bool {
    for t in 0..SINGLE_TRIBES.len() {
        let bit = 1 << t;
        if types[i] & bit == 0 || *tried & bit != 0 {
            continue;
        }
        *tried |= bit;
        if holder[t].is_none_or(|j| give_type(j, types, holder, tried)) {
            holder[t] = Some(i);
            return true;
        }
    }
    false
}

/// How many of the minions in `set` (bits over board indices) can each take a different type:
/// the size of a maximum matching of those minions to types.
fn matched_count(types: &[u16], set: u32) -> usize {
    let mut holder = [None; SINGLE_TRIBES.len()];
    (0..types.len())
        .filter(|&i| set >> i & 1 == 1 && give_type(i, types, &mut holder, &mut 0))
        .count()
}

/// How many ways the minions in `set` can each take a different type. All-type minions take any
/// of the types the others leave, so only the others' (at most 2 types each) are enumerated.
fn assignment_count(types: &[u16], set: u32) -> u64 {
    fn count(minions: &[usize], types: &[u16], used: u16) -> u64 {
        let Some((&i, rest)) = minions.split_first() else {
            return 1;
        };
        (0..SINGLE_TRIBES.len())
            .map(|t| 1 << t)
            .filter(|&bit| types[i] & !used & bit != 0)
            .map(|bit| count(rest, types, used | bit))
            .sum()
    }
    const ALL_TYPES: u16 = (1 << SINGLE_TRIBES.len()) - 1;
    let (all_type, others): (Vec<usize>, Vec<usize>) = (0..types.len())
        .filter(|&i| set >> i & 1 == 1)
        .partition(|&i| types[i] == ALL_TYPES);
    let left = SINGLE_TRIBES.len().saturating_sub(others.len());
    let all_type_ways: u64 = (0..all_type.len())
        .map(|k| left.saturating_sub(k) as u64)
        .product();
    count(&others, types, 0) * all_type_ways
}

/// The most types the living minions on `board` can stand for at once, each a different one (a
/// maximum matching of minions to types; `Menagerie Tableware`).
pub(crate) fn menagerie_type_count(board: &[Unit]) -> usize {
    let everyone = (0..board.len()).fold(0u32, |set, i| set | 1 << i);
    matched_count(&type_bits(board), everyone)
}

/// "A friendly minion of each type" (`Misplaced Tea Set`, `The Last One Standing`): the board
/// indices (in board order) of the living minions in the best assignment of minions to types, at
/// most one per type (a maximum matching; an All-type minion can take any type). Among the best
/// assignments, one is chosen uniformly at random.
pub(crate) fn select_menagerie_targets(board: &[Unit], rng: &mut Rng) -> Vec<usize> {
    let types = type_bits(board);
    let eligible = (0..board.len())
        .filter(|&i| types[i] != 0)
        .fold(0u32, |set, i| set | 1 << i);
    let best = matched_count(&types, eligible);
    let mut chosen = eligible;
    if best < eligible.count_ones() as usize {
        // The sets of `best` minions that can each take a different type, weighted by how many
        // best assignments give each one.
        let mut options: Vec<(u32, u64)> = Vec::new();
        let mut set = eligible;
        while set != 0 {
            if set.count_ones() as usize == best && matched_count(&types, set) == best {
                options.push((set, assignment_count(&types, set)));
            }
            set = (set - 1) & eligible;
        }
        chosen = options[0].0;
        if options.len() > 1 {
            let total: u64 = options.iter().map(|&(_, ways)| ways).sum();
            let mut pick = rng.below(total as usize) as u64;
            for &(set, ways) in &options {
                if pick < ways {
                    chosen = set;
                    break;
                }
                pick -= ways;
            }
        }
    }
    (0..board.len()).filter(|&i| chosen >> i & 1 == 1).collect()
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
        opt.combine_choose_one = true;
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

/// Unit id that marks the target of the spell being cast while its casts resolve, so that each
/// cast finds it wherever it moved (see [`cast_spell_on`]; no targeted spell is cast while another
/// one's casts resolve). Minions in the Tavern have no other use for their ids (combat assigns its
/// own).
const CAST_TARGET_ID: UnitId = UnitId::MAX;

/// Resolve the effect of casting `card` from `hand` at `board_pos` (0 if it is untargeted): see
/// [`cast_spell_on`].
pub fn cast_spell(
    state: &mut TavernState,
    card: Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    cast_spell_on(state, card, MinionSlot::Board(board_pos), pool, rng);
}

/// Resolve the effect of casting `card` from `hand` on `target` (ignored if it is untargeted;
/// `docs/tavern.md` §7.4). Each cast counts as a spell cast (`spells_played`) and runs the
/// `spell_cast` observers if it is a Tavern spell, then its `cast_at` / `cast` hook (a spell
/// without one adds its stats to the target), then the after-targeted-spell observers if its
/// target is still there (on the board or in the Tavern), then the after-cast observers. A
/// [`CardFlags::BOUNTY`] spell is cast [`Passive::BountyCasts`] times, and one cast on a friendly
/// minion [`Passive::TargetedSpellCasts`] times. The casts follow their target wherever it moves;
/// once it is gone (destroyed, sold, transformed, ...), the remaining casts fizzle: they do nothing.
pub fn cast_spell_on(
    state: &mut TavernState,
    card: Unit,
    target: MinionSlot,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let card_hooks = hooks(card.card_id);
    let targeted = card_hooks.spell_target != SpellTarget::None;
    let nested = targeted && state.board.iter().chain(&state.shop).any(is_cast_target);
    debug_assert!(
        !nested,
        "a targeted spell is cast while another one's casts resolve"
    );
    let target_id = if targeted {
        match state.minion_mut(target) {
            Some(unit) => Some(std::mem::replace(&mut unit.id, CAST_TARGET_ID)),
            None => return, // nothing to cast it on
        }
    } else {
        None
    };
    let tavern_spell = is_tavern_spell(card.card_id);
    if tavern_spell {
        state.auras.last_tavern_spell_cast = Some(card.card_id);
    }

    let mut casts = if card_hooks.has(CardFlags::BOUNTY) {
        board_passive(&state.board, Passive::BountyCasts)
    } else {
        1
    };
    let on_board = matches!(target, MinionSlot::Board(_));
    if targeted && on_board {
        casts *= board_passive(&state.board, Passive::TargetedSpellCasts);
    }

    for cast in 0..casts {
        if targeted && find_cast_target(state, target).is_none() {
            break; // the target is gone: this cast and the rest fizzle
        }
        state.auras.spells_played += 1;
        if tavern_spell {
            crate::cards::on_cast_tavern_spell(state, pool, rng);
        }
        let slot = if targeted {
            find_cast_target(state, target)
        } else {
            Some(target)
        };
        state.combine_choose_one = card.combine_choose_one;
        let outer_repeating = std::mem::replace(&mut state.repeating_cast, cast > 0);
        if let Some(slot) = slot {
            if let Some(cast_at) = card_hooks.cast_at {
                cast_at(state, &card, slot, pool, rng);
            } else if let Some(cast) = card_hooks.cast {
                let (MinionSlot::Board(pos) | MinionSlot::Shop(pos)) = slot;
                cast(state, &card, pos, pool, rng);
            } else if let Some(unit) = state.minion_mut(slot) {
                unit.add_stats(card.attack, card.health);
            }
        }
        state.repeating_cast = outer_repeating;
        state.combine_choose_one = false;
        if targeted {
            if let Some(slot) = find_cast_target(state, target) {
                crate::cards::after_cast_targeted_spell(state, slot, pool, rng);
            }
        }
        crate::cards::after_cast_any_spell(state, pool, rng);
    }

    if let Some(id) = target_id {
        unmark_cast_target(state, id);
    }
}

/// `true` if `unit` is marked as the target of the spell being cast (see [`CAST_TARGET_ID`]).
fn is_cast_target(unit: &Unit) -> bool {
    unit.id == CAST_TARGET_ID
}

/// Where the target of the spell being cast (marked with [`CAST_TARGET_ID`]) now is, in the zone
/// of `target` (`None` once it is gone).
fn find_cast_target(state: &TavernState, target: MinionSlot) -> Option<MinionSlot> {
    let (zone, slot): (&[Unit], fn(usize) -> MinionSlot) = match target {
        MinionSlot::Board(_) => (&state.board, MinionSlot::Board),
        MinionSlot::Shop(_) => (&state.shop, MinionSlot::Shop),
    };
    zone.iter().position(is_cast_target).map(slot)
}

/// Give the target of the spell cast (and any copy of it made meanwhile) its own id back.
fn unmark_cast_target(state: &mut TavernState, id: UnitId) {
    let prompts = state
        .discover_pending
        .iter_mut()
        .chain(&mut state.discover_queue)
        .flatten();
    let units = state
        .board
        .iter_mut()
        .chain(&mut state.hand)
        .chain(&mut state.shop)
        .chain(prompts);
    for unit in units.filter(|u| is_cast_target(u)) {
        unit.id = id;
    }
}

/// Record `stacks` of `spell`'s player effect, lasting `duration` (it acts through the spell's
/// `player_*` hooks, registered in [`behaviors`]).
fn record_effect(state: &mut TavernState, spell: CardId, stacks: u32, duration: EffectDuration) {
    state.auras.add_effect(spell, stacks, duration);
}

/// Hooks of an untargeted spell cast with `cast`.
pub(crate) fn spell(cast: CastFn) -> CardHooks {
    CardHooks::EMPTY.on_cast(cast)
}

/// Hooks of a spell cast with `cast` on a friendly minion.
pub(crate) fn targeted(cast: CastFn) -> CardHooks {
    spell(cast).with_spell_target(SpellTarget::Friendly)
}

/// Hooks of a spell cast with `cast` on a friendly minion of `tribe`.
pub(crate) fn targeted_tribe(tribe: Tribe, cast: CastFn) -> CardHooks {
    spell(cast).with_spell_target(SpellTarget::FriendlyTribe(tribe))
}

/// Hooks of a spell cast with `cast` on a friendly minion with a minion type (it uses the
/// target's type).
pub(crate) fn targeted_typed(cast: CastFn) -> CardHooks {
    spell(cast).with_spell_target(SpellTarget::FriendlyTyped)
}

/// Hooks of a spell cast with `cast` on any minion ("a minion"): a friendly one or one in the
/// Tavern.
pub(crate) fn targeted_minion(cast: CastAtFn) -> CardHooks {
    CardHooks::EMPTY
        .on_cast_at(cast)
        .with_spell_target(SpellTarget::Minion)
}

/// Behaviour tables for all spells, Tavern and token, from their modules (registered in the card
/// registry).
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    let mut out = vec![
        // Tier 1
        (SPELL_A_NEW_SPROUT, a_new_sprout::hooks()),
        (SPELL_ALLIANCE_FLAG, alliance_flag::hooks()),
        (SPELL_ENCHANTED_LASSO, enchanted_lasso::hooks()),
        (SPELL_FORTIFY, fortify::hooks()),
        (SPELL_RECRUIT_A_TRAINEE, recruit_a_trainee::hooks()),
        (SPELL_TAVERN_DISH_BANANA, tavern_dish_banana::hooks()),
        (SPELL_THEM_APPLES, them_apples::hooks()),
        // Tier 2
        (SPELL_CHEFS_CHOICE, chefs_choice::hooks()),
        (SPELL_HASTY_EXCAVATION, hasty_excavation::hooks()),
        (
            SPELL_LEAF_THROUGH_THE_PAGES,
            leaf_through_the_pages::hooks(),
        ),
        (SPELL_MIGHT_OF_STORMWIND, might_of_stormwind::hooks()),
        (SPELL_SEARCH_THROUGH_TIME, search_through_time::hooks()),
        (SPELL_STRIKE_OIL, strike_oil::hooks()),
        (SPELL_WINNERS_BREAD, winners_bread::hooks()),
        // Tier 3
        (SPELL_CAREFUL_INVESTMENT, careful_investment::hooks()),
        (SPELL_FRIENDLY_BOUNTY, friendly_bounty::hooks()),
        (SPELL_HEALTHY_BOUNTY, healthy_bounty::hooks()),
        (SPELL_HOSTILE_BOUNTY, hostile_bounty::hooks()),
        (SPELL_OVERCONFIDENCE, overconfidence::hooks()),
        (SPELL_PLANAR_TELESCOPE, planar_telescope::hooks()),
        (SPELL_REPAIR_JOB, repair_job::hooks()),
        (SPELL_ROBUST_EVOLUTION, robust_evolution::hooks()),
        (SPELL_SEAFOOD_STEW, seafood_stew::hooks()),
        (SPELL_SELFISH_BOUNTY, selfish_bounty::hooks()),
        (SPELL_SHINY_RING, shiny_ring::hooks()),
        (SPELL_STAFF_OF_ENRICHMENT, staff_of_enrichment::hooks()),
        (SPELL_TIME_MANAGEMENT, time_management::hooks()),
        (SPELL_TRICKY_TROUSERS, tricky_trousers::hooks()),
        (SPELL_WEALTHY_BOUNTY, wealthy_bounty::hooks()),
        // Tier 4
        (SPELL_BLOOD_GEM_BARRAGE, blood_gem_barrage::hooks()),
        (SPELL_BOON_OF_BEETLES, boon_of_beetles::hooks()),
        (SPELL_BOUNDLESS_POTENTIAL, boundless_potential::hooks()),
        (SPELL_CLONING_CONCH, cloning_conch::hooks()),
        (SPELL_DEFENDERS_RITES, defenders_rites::hooks()),
        (SPELL_EASTERLY_WINDS, easterly_winds::hooks()),
        (SPELL_EONARS_FAVOR, eonars_favor::hooks()),
        (SPELL_METHODICAL_MADNESS, methodical_madness::hooks()),
        (SPELL_MIGHTY_DRAGONBREATH, mighty_dragonbreath::hooks()),
        (SPELL_MISPLACED_TEA_SET, misplaced_tea_set::hooks()),
        (SPELL_NATURAL_BLESSING, natural_blessing::hooks()),
        (SPELL_TEMPERATURE_SHIFT, temperature_shift::hooks()),
        (SPELL_TOMB_TURNING, tomb_turning::hooks()),
        (SPELL_WEAPONS_FORGE, weapons_forge::hooks()),
        // Tier 5
        (SPELL_ARMOR_STASH, armor_stash::hooks()),
        (SPELL_BROOD_OF_NOZDORMU, brood_of_nozdormu::hooks()),
        (SPELL_BUTCHERING, butchering::hooks()),
        (SPELL_CHANNEL_THE_DEVOURER, channel_the_devourer::hooks()),
        (SPELL_CONTRACTED_CORPSE, contracted_corpse::hooks()),
        (SPELL_CORRUPTED_COIN, corrupted_coin::hooks()),
        (SPELL_CORRUPTED_CUPCAKES, corrupted_cupcakes::hooks()),
        (SPELL_ENERGIZING_CHAMBER, energizing_chamber::hooks()),
        (SPELL_FORESTS_BOUNTY, forests_bounty::hooks()),
        (SPELL_HIRED_HEADHUNTER, hired_headhunter::hooks()),
        (SPELL_SALOONS_FINEST, saloons_finest::hooks()),
        (SPELL_UNMASKED_IDENTITY, unmasked_identity::hooks()),
        (SPELL_UPPER_HAND, upper_hand::hooks()),
        (SPELL_WAVE_OF_GOLD, wave_of_gold::hooks()),
        // Tier 6
        (SPELL_AZERITE_EMPOWERMENT, azerite_empowerment::hooks()),
        (
            SPELL_EYES_OF_THE_EARTH_MOTHER,
            eyes_of_the_earth_mother::hooks(),
        ),
        (SPELL_FANDRALS_FORTUNE, fandrals_fortune::hooks()),
        (SPELL_LOST_STAFF_OF_HAMUUL, lost_staff_of_hamuul::hooks()),
        (SPELL_PERFECT_VISION, perfect_vision::hooks()),
        // Tier 7
        (SPELL_HALLOWED_RITUAL, hallowed_ritual::hooks()),
        (SPELL_MENAGERIE_TABLEWARE, menagerie_tableware::hooks()),
        (SPELL_SACRED_GIFT, sacred_gift::hooks()),
        (SPELL_SHARING_IS_CARING, sharing_is_caring::hooks()),
        // Token spells
        (tokens::SPELL_ARCANE_ABSORPTION, arcane_absorption::hooks()),
        (tokens::SPELL_BLOOD_GEM, blood_gem::hooks()),
        (tokens::SPELL_CONFLAGRATION, conflagration::hooks()),
        (tokens::SPELL_GEM_CONFISCATION, gem_confiscation::hooks()),
        (tokens::SPELL_GEM_DAY, gem_day::hooks()),
        (tokens::SPELL_GOLDEN_TOUCH, golden_touch::hooks()),
        (tokens::SPELL_LOCKBOX, lockbox::hooks()),
        (tokens::SPELL_POINTY_ARROW, pointy_arrow::hooks()),
        (tokens::SPELL_SLUDGE_CORROSION, sludge_corrosion::hooks()),
        (tokens::SPELL_TAVERN_COIN, tavern_coin::hooks()),
    ];
    for (card_id, card_hooks) in &mut out {
        if BOUNTY_SPELL_IDS.contains(card_id) {
            *card_hooks = card_hooks.with_flags(CardFlags::BOUNTY);
        }
    }
    out
}
