//! `Galewing` (`BG20_HERO_283`) — Armor 10 (Duos 8).
//!
//! Hero Power: `Dungar's Gryphon` (`BG20_HERO_283p`), 0 Gold: Choose a new flightpath. Complete it to get a bonus!
//!
//! Active, untargeted, once per turn, while no flightpath is under way: a Choose One of the three
//! flightpaths, which becomes the Hero Power (its card, `BG20_HERO_283p_t1` .. `t3`; `counter` =
//! the turns left). At each turn start one turn passes (`hero_turn_start`); at 0 the bonus is
//! given and the power is *Dungar's Gryphon* again, usable that turn (engine decision):
//!
//! - **Westfall** (1 turn): a random 1-Cost Tavern spell;
//! - **Ironforge** (2 turns): 2 Gold;
//! - **Eastern Plaguelands** (3 turns): a Discover among 3 minions of exactly the player's Tier
//!   from the pool.
//!
//! A card needs room in the hand.

use crate::cards::{spells, tokens, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 75702;
pub const POWER: CardId = 75703;

/// The flightpaths' Hero Power cards, which are also their Choose One options.
pub const WESTFALL: CardId = 75704;
pub const IRONFORGE: CardId = 75705;
pub const EASTERN_PLAGUELANDS: CardId = 75706;

/// Each flightpath: its card, name and turns.
const PATHS: [(CardId, &str, i32); 3] = [
    (WESTFALL, "Westfall", 1),
    (IRONFORGE, "Ironforge", 2),
    (EASTERN_PLAGUELANDS, "Eastern Plaguelands", 3),
];

/// Ironforge's Gold.
const GOLD: u32 = 2;

/// Discover options.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, its flightpaths' and their options'.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    let path = |chosen| CardHooks::EMPTY.on_chosen(chosen).on_hero_turn_start(fly);
    vec![
        (POWER, CardHooks::EMPTY.on_hero_power(offer_paths)),
        (WESTFALL, path(choose_westfall)),
        (IRONFORGE, path(choose_ironforge)),
        (EASTERN_PLAGUELANDS, path(choose_eastern_plaguelands)),
    ]
}

/// Using it: choose a flightpath.
fn offer_paths(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    let options = PATHS
        .iter()
        .map(|&(id, name, _)| tokens::make_choice_option(id, name, false))
        .collect();
    state.push_discover(options);
}

/// The chosen flightpath becomes the Hero Power, with its turns left.
fn take_path(state: &mut TavernState, path: CardId) {
    let turns = PATHS.iter().find(|p| p.0 == path).map_or(1, |p| p.2);
    if let Some(power) = power_mut(&mut state.auras) {
        power.card_id = path;
        power.counter = turns;
    }
}

fn choose_westfall(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    take_path(state, WESTFALL);
}

fn choose_ironforge(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    take_path(state, IRONFORGE);
}

fn choose_eastern_plaguelands(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    take_path(state, EASTERN_PLAGUELANDS);
}

/// Start of turn: one turn passes; at 0, the bonus, and the power is Dungar's Gryphon again.
fn fly(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter -= 1;
    if power.counter > 0 {
        return;
    }
    let path = power.card_id;
    power.card_id = POWER;
    power.counter = 0;
    power.uses_left = 1;
    match path {
        WESTFALL => state.add_to_hand(spells::draw_random_cost_tavern_spell(1, rng)),
        IRONFORGE => state.gold += GOLD,
        _ => {
            let options = pool.draw_discover_options(state.tavern_tier, OPTIONS, rng);
            state.push_discover(options);
        }
    }
}
