//! `Quilligraphy Set (Greater)` (`BG30_MagicItem_410t2`) — Greater, 0 Gold, Quilboar.
//!
//! Avenge (4): Your Blood Gems give an extra +1/+1 this game.
//!
//! As the Lesser one (`quilligraphy_set.rs`): Avenge (4), +1/+1.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Unit};

pub const ID: CardId = 115253;

/// Friendly deaths per Avenge.
const AVENGE: i32 = 4;

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
        ctx.auras.blood_gem_bonus_atk += 1;
        ctx.auras.blood_gem_bonus_hp += 1;
    }
}
