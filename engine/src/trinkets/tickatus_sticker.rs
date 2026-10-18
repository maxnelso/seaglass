//! `Tickatus Sticker` (`BG30_MagicItem_707`) — Lesser, 2 Gold.
//!
//! Discover a Tier 3 Darkmoon Prize. At the start of every three turns, repeat this. (3 turns left!)
//!
//! Picked: a Tier 3 prize Discover (`darkmoon::discover_prize`, `docs/darkmoon_prizes.md` §7.2);
//! then every 3rd turn start after the pick (`counter`: turns 9, 12, ... for one picked on turn
//! 6), another.

use crate::cards::CardHooks;
use crate::darkmoon;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 117860;

/// The prize Tier.
const TIER: u32 = 3;
/// Turns between prizes.
const TURNS: i32 = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(discover)
            .on_trinket_turn_start(turn_start),
    )]
}

fn discover(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    darkmoon::discover_prize(state, TIER, pool, rng);
}

fn turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if super::count_to(&mut state.auras, ID, TURNS) {
        discover(state, pool, rng);
    }
}
