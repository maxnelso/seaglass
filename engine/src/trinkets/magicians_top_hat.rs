//! `Magician's Top Hat` (`BG35_MagicItem_815`) — Lesser, 2 Gold.
//!
//! Get two minions each from Tiers 1, 2, and 3.
//!
//! Picked: 2 random Tier 1 minions, then 2 of Tier 2, then 2 of Tier 3, from the game's catalog
//! (`trinkets::give_random_minion_of_tier`, one draw each), each to the hand or waiting for room
//! in a full hand.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130833;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    for tier in [1, 1, 2, 2, 3, 3] {
        super::give_random_minion_of_tier(state, rng, tier);
    }
}
