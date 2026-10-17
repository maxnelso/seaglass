//! `Furtiveness` (`BG36_MidGameEffect_000t79`) — turns 4–, weight 100.
//!
//! Stealth
//!
//! Given: Stealth. Only on Avenge minions (`docs/dark_gifts.md` §5.2).

use crate::cards::{CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, DarkGift, Keyword, Unit};

pub const ID: CardId = 133472;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_given(given)
            .on_gift_offerable(offerable),
    )]
}

fn given(unit: &mut Unit, _: &mut DarkGift) {
    unit.apply_keyword(Keyword::Stealth, false);
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    super::is_avenge(t)
}
