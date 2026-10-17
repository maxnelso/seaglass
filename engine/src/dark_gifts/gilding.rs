//! `Gilding` (`BG36_MidGameEffect_000t14`) — turns 4–8, weight 100.
//!
//! This is Golden, but doesn't give a Triple Reward.
//!
//! Given: made Golden as an intrinsic Golden (`dark_gifts::make_intrinsic_golden`): no Triple
//! Reward, one pool copy, never in a Triple. A gift for Battlecry minions; only on the lowest-Tier
//! minion offered, and not on Activate minions (`docs/dark_gifts.md` §5.2).

use crate::cards::{CardFlags, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, DarkGift, Unit};

pub const ID: CardId = 132441;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_FOR_BATTLECRY)
            .on_gift_given(given)
            .on_gift_offerable(offerable),
    )]
}

fn given(unit: &mut Unit, _: &mut DarkGift) {
    super::make_intrinsic_golden(unit);
}

fn offerable(ctx: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    t.tavern_tier <= ctx.lowest_tier && !super::is_activate(t)
}
