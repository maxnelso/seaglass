//! `Demonology` (`BG36_MidGameEffect_000t66`) — turns 5–8, weight 200.
//!
//! Rally: Add a Fodder to your next 3 Refreshes.
//!
//! Rally: a Demon Fodder is added to each of the player's next 3 Refreshes
//! (`tokens::add_fodder_to_refreshes`). Only on Demons (`docs/dark_gifts.md` §5.2).

use crate::cards::{tokens, CardHooks, CardTemplate, RallyCtx};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 133351;

/// The Refreshes that get a Fodder.
const REFRESHES: u32 = 3;

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
    tokens::add_fodder_to_refreshes(ctx.auras, 1, REFRESHES);
    Vec::new()
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    t.tribe.matches(Tribe::Demon)
}
