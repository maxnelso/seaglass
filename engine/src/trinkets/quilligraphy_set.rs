//! `Quilligraphy Set` (`BG30_MagicItem_410`) — Lesser, 0 Gold, Quilboar.
//!
//! Avenge (3): Your Blood Gems give an extra +1 Health this game.
//!
//! `trinket_friendly_death`: every 3rd friendly minion that dies in a combat (the count starts
//! again each combat: confirmed) raises the Blood Gem bonus (`auras.blood_gem_bonus_hp`, as `Gem
//! Day`'s) by 1 for the rest of the game.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Unit};

pub const ID: CardId = 111512;

/// Friendly deaths per Avenge.
const AVENGE: i32 = 3;

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
        ctx.auras.blood_gem_bonus_hp += 1;
    }
}
