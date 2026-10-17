//! `Sunken Persistence` (`BG36_MidGameEffect_000t62`) — turns 3–, weight 200.
//!
//! This minion's Spellcrafts are permanent.
//!
//! The Spellcraft spell it gives carries the gift (`CardFlags::GIFT_KEEPS_SPELLCRAFT`), so it is
//! not temporary: it stays in the hand at the end of the turn (engine decision;
//! `docs/dark_gifts.md` §11.2). Only on Spellcraft minions (§5.2): none is in a pool of Patch
//! 36.6.3.

use crate::cards::{CardFlags, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::CardId;

pub const ID: CardId = 133310;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_KEEPS_SPELLCRAFT)
            .on_gift_offerable(offerable),
    )]
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    super::is_spellcraft(t)
}
