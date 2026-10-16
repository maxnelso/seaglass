//! `Designer Eyepatch` (`BG30_MagicItem_439`) — Greater, 7 Gold, Pirate.
//!
//! You only need 2 copies of a Pirate to make it Golden.
//!
//! `trinket_triple_copies`: 2 copies of a Pirate form a Triple (`TavernState::triple_copies_for`,
//! as Mister Clocksworth's rule does for every minion; the fewest copies any rule asks for).

use crate::cards::{self, CardHooks};
use crate::model::{CardId, Tribe};

pub const ID: CardId = 112990;

/// The copies of a Pirate a Triple takes.
const COPIES: usize = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_triple_copies(copies))]
}

fn copies(card_id: CardId) -> Option<usize> {
    cards::template(card_id)
        .filter(|t| t.tribe.matches(Tribe::Pirate))
        .map(|_| COPIES)
}
