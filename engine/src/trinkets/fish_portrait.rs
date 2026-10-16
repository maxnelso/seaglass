//! `Fish Portrait` (`BG30_MagicItem_821`) — Lesser, 1 Gold, Beast, Undead.
//!
//! Get a Fish of N'Zoth.
//!
//! Picked: a `Fish of N'Zoth` (N'Zoth's `2/2` Beast token, which gains the Deathrattles of the
//! friendly minions that die in combat: `heroes/nzoth.rs`) goes to the hand, or waits for room in
//! a full hand.

use crate::cards::{tokens, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 110966;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.add_to_hand_or_wait(tokens::make_fish_of_nzoth());
}
