//! `Persisting Horror` (`BG36_MidGameEffect_000t12`) — turns 10–, weight 50.
//!
//! Reborn. Is Reborn with full stats and Bonus Keywords.
//!
//! Given: Reborn. Its Reborn copy is the minion as it died, with its Attack, its full Health
//! (`max_health`), its keywords but Reborn, and its gifts (engine decision). Not on minions with no
//! type (`docs/dark_gifts.md` §5.2).

use crate::cards::{CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, DarkGift, Keyword, Unit};

pub const ID: CardId = 132276;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_given(given)
            .on_gift_reborn(reborn)
            .on_gift_offerable(offerable),
    )]
}

fn given(unit: &mut Unit, _: &mut DarkGift) {
    unit.apply_keyword(Keyword::Reborn, false);
}

fn reborn(dying: &Unit, copy: &mut Unit) {
    let mut full = dying.clone();
    full.id = copy.id;
    full.health = dying.max_health.max(1);
    full.reborn = false;
    full.temp_reborn = false;
    full.killed_by = None;
    full.divine_shield |= dying.inherent_divine_shield;
    *copy = full;
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    super::has_type(t)
}
