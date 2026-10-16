//! `Ghastly Sticker` (`BG32_MagicItem_367`) — Greater, 4 Gold.
//!
//! Your end of turn effects trigger an extra time.
//!
//! `trinket_extra_triggers`: +1 to the end-of-turn repeat count (`cards::end_of_turn_repeats`,
//! `docs/tavern.md` §7.5) of every end-of-turn effect: the minions', the Hero Power's, the
//! trinkets' (confirmed); added to `Drakkari Enchanter`'s, 3 times with both (confirmed). Turn
//! counters advance once and charges are used once (confirmed).

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
