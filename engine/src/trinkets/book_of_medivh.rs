//! `Book of Medivh` (`BG30_MagicItem_420`) — Lesser, 1 Gold.
//!
//! Discover a Tavern spell. At the start of each turn, Discover another.
//!
//! As picked and at each turn start: a Discover of 3 Tavern spells of the player's Tier or lower
//! (`trinkets::discover_tavern_spells`, as `Clever Castaway`'s).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 111609;

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
    super::discover_tavern_spells(state, 1, rng);
}
