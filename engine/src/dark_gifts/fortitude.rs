//! `Fortitude` (`BG36_MidGameEffect_000t73`) — turns 3–3, weight 100.
//!
//! +5/+5.
//!
//! Given: +5/+5 (the post's +4/+4 before Patch 36.6.3). A stat gift (`docs/dark_gifts.md` §11.1).

use crate::cards::{CardFlags, CardHooks};
use crate::model::{CardId, DarkGift, Unit};

pub const ID: CardId = 133421;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_GIVES_STATS)
            .on_gift_given(given),
    )]
}

fn given(unit: &mut Unit, _: &mut DarkGift) {
    unit.add_stats(5, 5);
}
