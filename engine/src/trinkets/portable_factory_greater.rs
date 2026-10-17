//! `Portable Factory (Greater)` (`BG32_MagicItem_361t`) — Greater, 0 Gold.
//!
//! Discover a Tier 5 minion with a type. At the start of each turn, get another copy of it.
//!
//! As the Lesser one, Tier 5.

use crate::cards::CardHooks;
use crate::model::CardId;

pub const ID: CardId = 120925;

/// The Tier offered.
const TIER: u32 = 5;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(|s, p, r| super::factory_discover(s, ID, TIER, p, r))
            .on_trinket_after_discover(|s, c, o, _, _| super::factory_chosen(s, ID, c, o))
            .on_trinket_turn_start(|s, _, _| super::factory_copy(s, ID)),
    )]
}
