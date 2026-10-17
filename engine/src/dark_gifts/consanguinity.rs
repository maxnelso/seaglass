//! `Consanguinity` (`BG36_MidGameEffect_000t80`) — turns 3–5, weight 200.
//!
//! Rally: Get 2 Blood Gems.
//!
//! Rally: 2 `Blood Gem`s to the hand (the combat hand in combat). Only on Quilboar
//! (`docs/dark_gifts.md` §5.2).

use crate::cards::{tokens, CardHooks, CardTemplate, RallyCtx};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 133474;

/// Blood Gems it gives.
const GEMS: usize = 2;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_rally(rally)
            .on_gift_offerable(offerable),
    )]
}

fn rally(ctx: &mut RallyCtx<'_>) -> Vec<Unit> {
    for _ in 0..GEMS {
        ctx.generated_hand.push(tokens::make_blood_gem());
    }
    Vec::new()
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    t.tribe.matches(Tribe::Quilboar)
}
