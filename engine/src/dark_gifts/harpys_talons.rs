//! `Harpy's Talons` (`BG36_MidGameEffect_000t13`) — turns –, weight 100.
//!
//! Divine Shield, Windfury
//!
//! Given: Divine Shield and Windfury (`docs/dark_gifts.md` §11.1). Offered more often on Rally
//! minions: its weight is doubled on one (engine decision, §4.2).

use crate::cards::{CardHooks, CardTemplate};
use crate::model::{CardId, DarkGift, Keyword, Unit};

pub const ID: CardId = 132279;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_given(given)
            .on_gift_weight_factor(weight_factor),
    )]
}

fn given(unit: &mut Unit, _: &mut DarkGift) {
    unit.apply_keyword(Keyword::DivineShield, false);
    unit.apply_keyword(Keyword::Windfury, false);
}

fn weight_factor(t: &CardTemplate) -> u32 {
    if super::is_rally(t) {
        2
    } else {
        1
    }
}
