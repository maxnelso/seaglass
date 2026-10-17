//! `Fresh Perspective` (`BG36_MidGameEffect_000t52`) — turns 3–5, weight 100.
//!
//! Deathrattle: Gain 2 free Refreshes.
//!
//! Deathrattle: the player gains 2 free Refreshes (`auras.free_refreshes`), which carry over
//! (`docs/dark_gifts.md` §11.1).

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, DarkGift, Unit};

pub const ID: CardId = 132734;

/// Free Refreshes it gives.
const REFRESHES: u32 = 2;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_gift_deathrattle(deathrattle))]
}

fn deathrattle(_: &Unit, _: &DarkGift, ctx: &mut BoardCtx<'_>) {
    ctx.auras.free_refreshes += REFRESHES;
}
