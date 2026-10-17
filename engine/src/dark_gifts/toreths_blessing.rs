//! `Toreth's Blessing` (`BG36_MidGameEffect_000t15`) — turns 5–, weight 200.
//!
//! This minion's Divine Shield takes 3 hits to break.
//!
//! Each combat, its Divine Shield absorbs 2 hits without breaking (`gift_absorb_shield_hit`,
//! restarted by `gift_combat_start`); the third breaks it. Only on Divine Shield minions
//! (`docs/dark_gifts.md` §5.2).

use crate::cards::{CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, DarkGift};

pub const ID: CardId = 132442;

/// Hits the shield absorbs before the one that breaks it.
const EXTRA_HITS: i32 = 2;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_combat_start(|gift| gift.counter = EXTRA_HITS)
            .on_gift_absorb_shield_hit(absorb)
            .on_gift_offerable(offerable),
    )]
}

fn absorb(gift: &mut DarkGift) -> bool {
    if gift.counter > 0 {
        gift.counter -= 1;
        true
    } else {
        false
    }
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    t.divine_shield
}
