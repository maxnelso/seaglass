//! `Toxicity` (`BG36_MidGameEffect_000t69`) — turns 9–, weight 100.
//!
//! Venomous
//!
//! Given: Venomous. Only on Murlocs, and not on minions that already have Venomous
//! (`docs/dark_gifts.md` §5.2).

use crate::cards::{CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::Tribe;
use crate::model::{CardId, DarkGift, Keyword, Unit};
pub const ID: CardId = 133353;

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
    unit.apply_keyword(Keyword::Venomous, false);
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    t.tribe.matches(Tribe::Murloc) && !t.venomous
}
