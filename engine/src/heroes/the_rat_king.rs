//! `The Rat King` (`TB_BaconShop_HERO_12`) — Armor 14 (Duos 12).
//!
//! Hero Power: `A Tale of Kings` (`TB_BaconShop_HP_041`), 2 Gold: Discover a minion of a specific minion type. Swaps type each turn.
//!
//! Active, untargeted, once per turn. The type of turn `t` is the `(t - 1) mod 10`-th lobby type
//! (`LOBBY_TRIBES`: Aberration on turn 1, then Beast, Demon, ..., Undead, and again), set at each
//! turn start (`counter` = its index; engine decision). Using it Discovers among 3 minions of that
//! type of the player's Tier or lower from the pool, as Alexstrasza's; it needs room in the hand.

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, LOBBY_TRIBES};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 57893;
pub const POWER: CardId = 63127;

/// Discover options.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(a_tale_of_kings)
            .on_hero_power_usable(super::hand_has_room)
            .on_hero_turn_start(turn_start),
    )]
}

/// Start of turn: the turn's type.
fn turn_start(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let index = (state.turn.max(1) - 1) as usize % LOBBY_TRIBES.len();
    if let Some(power) = power_mut(&mut state.auras) {
        power.counter = index as i32;
    }
}

/// Using it: Discover a minion of the turn's type.
fn a_tale_of_kings(state: &mut TavernState, _: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    let index = state.auras.hero.as_ref().map_or(0, |h| h.power.counter) as usize;
    let tribe = LOBBY_TRIBES[index % LOBBY_TRIBES.len()];
    let options = pool.draw_discover_by_tribe(tribe, state.tavern_tier, OPTIONS, rng);
    state.push_discover(options);
}
