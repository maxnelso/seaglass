//! `Book of Medivh (Greater)` (`BG30_MagicItem_420t`) — Greater, 2 Gold.
//!
//! Discover 2 Tavern spells. At the start of each turn, Discover 2 more.
//!
//! As the Lesser one, 2 Discovers each time.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 115230;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(discover)
            .on_trinket_turn_start(discover),
    )]
}

fn discover(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    super::discover_tavern_spells(state, 2, rng);
}
