//! `Conch Portrait` (`BG35_MagicItem_305`) — Lesser, 5 Gold, Murloc.
//!
//! Get a Cloning Conch. At the start of every 2 turns, repeat this. (2 turns left!)
//!
//! Picked: a `Cloning Conch` goes to the hand, or waits for room in a full hand; then every 2nd
//! turn start (`counter`: turn 8, 10, ... for one picked on turn 6), another.

use crate::cards::{spells, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131137;

/// Turns between Conches.
const TURNS: i32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(give)
            .on_trinket_turn_start(turn_start),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if let Some(conch) = spells::spell_by_id(spells::SPELL_CLONING_CONCH) {
        state.add_to_hand_or_wait(conch);
    }
}

fn turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if super::count_to(&mut state.auras, ID, TURNS) {
        give(state, pool, rng);
    }
}
