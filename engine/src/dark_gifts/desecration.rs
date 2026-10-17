//! `Desecration` (`BG36_MidGameEffect_000t88`) — turns 5–7, weight 100.
//!
//! Has +3/+3 for each card you've discarded this game. ({1})
//!
//! `gift_aura`: +3/+3 for each card the player discarded this game (`auras.cards_discarded`),
//! wherever it is. Only once the player discarded at least one card (confirmed;
//! `docs/dark_gifts.md` §5.2). A stat gift.

use crate::cards::{CardFlags, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, DarkGift, PlayerAuras, Unit};

pub const ID: CardId = 134731;

/// The stats it has for each one.
const PER: i32 = 3;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_GIVES_STATS)
            .on_gift_aura(aura)
            .on_gift_offerable(offerable),
    )]
}

fn count(auras: &PlayerAuras) -> u32 {
    auras.cards_discarded
}

fn aura(_: &Unit, _: &DarkGift, auras: &PlayerAuras) -> (i32, i32) {
    let bonus = PER.saturating_mul(count(auras).min(i32::MAX as u32) as i32);
    (bonus, bonus)
}

fn offerable(ctx: &OfferCtx<'_>, _: &CardTemplate) -> bool {
    ctx.auras.cards_discarded > 0
}
