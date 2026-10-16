//! `Ghastly Sticker` (`BG32_MagicItem_367`) — Greater, 4 Gold.
//!
//! Your end of turn effects trigger an extra time.
//!
//! `trinket_extra_triggers`: the minions' end-of-turn triggers (`tavern.md` §7.5) resolve one more
//! time, on top of `Drakkari Enchanter`'s (**engine decision**: the minions' only, not a Hero
//! Power's or a trinket's).

use crate::cards::{CardHooks, Passive};
use crate::model::CardId;

pub const ID: CardId = 120873;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_extra_triggers(extra))]
}

fn extra(passive: Passive) -> u32 {
    u32::from(passive == Passive::EndOfTurnTriggers)
}
