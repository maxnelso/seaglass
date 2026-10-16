//! `Dragon's Eye` (`BG36_MagicItem_215`) — Greater, 4 Gold, Dragon.
//!
//! Your Dragons' Battlecries trigger an extra time.
//!
//! `trinket_extra_battlecries`: a Dragon's Battlecry (played or triggered in the Tavern) triggers
//! once more, added to `Brann Bronzebeard`'s (3 times with both: **engine decision**).

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, TrinketState, Unit};

pub const ID: CardId = 133403;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_extra_battlecries(extra))]
}

fn extra(_: &mut TrinketState, unit: &Unit) -> u32 {
    u32::from(unit.tribe.matches(Tribe::Dragon))
}
