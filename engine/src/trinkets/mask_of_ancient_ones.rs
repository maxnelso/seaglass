//! `Mask of Ancient Ones` (`BG36_MagicItem_602`) — Greater, 4 Gold.
//!
//! Make your Deity Golden this game.
//!
//! Picked: the player's Deity (`auras.deity`, `DeityState::is_golden`) is Golden for the rest of
//! the game, with what a Golden Deity does (`cards/deities.rs`).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134322;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.auras.deity.is_golden = true;
}
