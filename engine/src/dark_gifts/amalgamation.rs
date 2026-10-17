//! `Amalgamation` (`BG36_MidGameEffect_000t22`) — turns 8–, weight 200.
//!
//! Has all minion types.
//!
//! Given: its type becomes `All`, which a Golden a Triple makes keeps (`gift_reapply`). Only on
//! minions with no type (`docs/dark_gifts.md` §5.2).

use crate::cards::{CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, DarkGift, Tribe, Unit};

pub const ID: CardId = 132790;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_given(|unit, _: &mut DarkGift| all_types(unit))
            .on_gift_reapply(all_types)
            .on_gift_offerable(offerable),
    )]
}

fn all_types(unit: &mut Unit) {
    unit.tribe = Tribe::All;
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    !super::has_type(t)
}
