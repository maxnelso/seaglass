//! `Tarecgosa's Blessing` (`BG36_MidGameEffect_000t50`) — turns 6–, weight 200.
//!
//! Permanently keeps Bonus Keywords and double stats gained in combat.
//!
//! It keeps twice the stats it gained in combat and the Bonus Keywords it gained there
//! (`gift_keep_combat_mult` 2, `cards::keep_combat_gains`). Only on Dragons (`docs/dark_gifts.md`
//! §5.2).

use crate::cards::{CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, Tribe};

pub const ID: CardId = 132732;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_keep_combat_mult(|_| 2)
            .on_gift_offerable(offerable),
    )]
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    t.tribe.matches(Tribe::Dragon)
}
