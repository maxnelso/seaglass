//! `Explorer's Binoculars` (`BG32_MagicItem_858`) — Lesser, 1 Gold.
//!
//! Get three random Tier 4 minions.
//!
//! Picked: 3 random Tier 4 minions (exactly Tier 4, whatever the Tavern's Tier) from the game's
//! catalog (`trinkets::give_random_minion_of_tier`, one draw each), each to the hand or waiting
//! for room in a full hand.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 120139;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    for _ in 0..3 {
        super::give_random_minion_of_tier(state, rng, 4);
    }
}
