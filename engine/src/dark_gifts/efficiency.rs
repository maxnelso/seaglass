//! `Efficiency` (`BG36_MidGameEffect_000t78`) — turns 5–, weight 200.
//!
//! This minion's Activate triggers twice.
//!
//! `gift_extra_activations`: its `Activate` triggers once more, for one price. Only on Activate
//! minions (confirmed; `docs/dark_gifts.md` §5.2).

use crate::cards::{CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::CardId;

pub const ID: CardId = 133470;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_extra_activations(|_| 1)
            .on_gift_offerable(offerable),
    )]
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    super::is_activate(t)
}
