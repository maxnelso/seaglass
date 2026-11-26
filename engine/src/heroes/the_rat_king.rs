//! `The Rat King` (`TB_BaconShop_HERO_12`) — Armor 14 (Duos 12).
//!
//! Hero Power: `A Tale of Kings` (`TB_BaconShop_HP_041`), 2 Gold: Discover a minion of a specific minion type. Swaps type each turn.
//!
//! Active, untargeted, once per turn. Rotates each turn through the minion types actually present
//! in the lobby (`LOBBY_TRIBES` filtered to active lobby types), storing the chosen type's index
//! in `LOBBY_TRIBES` in `power.counter`. Using it Discovers among 3 minions of that type of the
//! player's Tier or lower from the pool, as Alexstrasza's; it needs room in the hand.

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

/// Active lobby tribe indices (in `LOBBY_TRIBES` order) available in this lobby/pool.
fn active_tribe_indices(state: &TavernState, pool: &CardPool) -> Vec<usize> {
    let in_lobby = |t| state.lobby.tribes.is_empty() || state.lobby.tribes.contains(&t);
    let active: Vec<usize> = LOBBY_TRIBES
        .iter()
        .enumerate()
        .filter(|(_, &t)| in_lobby(t) && pool.lobby_has(t))
        .map(|(i, _)| i)
        .collect();
    if !active.is_empty() {
        return active;
    }
    let fallback: Vec<usize> = LOBBY_TRIBES
        .iter()
        .enumerate()
        .filter(|(_, &t)| in_lobby(t))
        .map(|(i, _)| i)
        .collect();
    if !fallback.is_empty() {
        return fallback;
    }
    (0..LOBBY_TRIBES.len()).collect()
}

/// Start of turn: rotate among the lobby's active types.
fn turn_start(state: &mut TavernState, pool: &mut CardPool, _: &mut Rng) {
    let active = active_tribe_indices(state, pool);
    let pick = active[(state.turn.max(1) - 1) as usize % active.len()];
    if let Some(power) = power_mut(&mut state.auras) {
        power.counter = pick as i32;
    }
}

/// Using it: Discover a minion of the turn's type.
fn a_tale_of_kings(state: &mut TavernState, _: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    let raw_idx = state.auras.hero.as_ref().map_or(0, |h| h.power.counter).max(0) as usize;
    let mut tribe = LOBBY_TRIBES[raw_idx % LOBBY_TRIBES.len()];
    if !pool.lobby_has(tribe) {
        let active = active_tribe_indices(state, pool);
        let pick = active[(state.turn.max(1) - 1) as usize % active.len()];
        tribe = LOBBY_TRIBES[pick];
        if let Some(power) = power_mut(&mut state.auras) {
            power.counter = pick as i32;
        }
    }
    let options = pool.draw_discover_by_tribe(tribe, state.tavern_tier, OPTIONS, rng);
    state.push_discover(options);
}
