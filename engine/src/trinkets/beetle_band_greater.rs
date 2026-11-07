//! `Beetle Band (Greater)` (`BG32_MagicItem_860t`) — Greater, 4 Gold, Beast.
//!
//! Avenge (8): Summon two 2/2 Beetles. Give them Taunt.
//!
//! As the Lesser `Beetle Band`: every 8th friendly minion that dies in a combat summons two
//! Beetles with Taunt.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Unit};

pub const ID: CardId = 121182;

/// Friendly deaths per Avenge.
const AVENGE: i32 = 8;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_combat_start(super::reset_per_combat)
            .on_trinket_friendly_death(avenge),
    )]
}

fn avenge(ctx: &mut BoardCtx<'_>, _: &Unit) {
    if super::avenge(ctx, ID, AVENGE) {
        super::beetle_band::summon_beetles(ctx, 2, "Beetle Band");
    }
}
