//! `Portable Factory` (`BG32_MagicItem_361`) — Lesser, 5 Gold.
//!
//! Discover a Tier 4 minion with a type. At the start of each turn, get another copy of it.
//!
//! As picked, a Discover of minions of Tier 4 with a type, from the pool; the trinket keeps the
//! options (`stored`, `armed` 1) until that Discover is resolved (`trinket_after_discover`: the
//! prompt whose cards are the kept ones), then a plain copy of the chosen minion. At each turn
//! start, a plain copy of it (a new copy, not from the pool).

use crate::cards::CardHooks;
use crate::model::CardId;

pub const ID: CardId = 111399;

/// The Tier offered.
const TIER: u32 = 4;

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
