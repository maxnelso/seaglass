//! `Titanic Strength` (`BG36_MidGameEffect_000t72`) — turns 11–, weight 50.
//!
//! +1000 Attack.
//!
//! Given: +1000 Attack. Not on Dragons (`docs/dark_gifts.md` §5.2). A stat gift.

use crate::cards::{CardFlags, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::Tribe;
use crate::model::{CardId, DarkGift, Unit};
pub const ID: CardId = 133363;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_GIVES_STATS)
            .on_gift_given(given)
            .on_gift_offerable(offerable),
    )]
}

fn given(unit: &mut Unit, _: &mut DarkGift) {
    unit.add_stats(1000, 0);
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    !t.tribe.matches(Tribe::Dragon)
}
