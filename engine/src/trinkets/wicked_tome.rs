//! `Wicked Tome` (`BG32_MagicItem_270t`) — Greater, 5 Gold, Undead.
//!
//! Avenge (4): Your Tavern spells give an extra +1/+1 this game.
//!
//! `trinket_friendly_death`: every 4th friendly minion that dies in a combat (the count starts
//! again each combat: confirmed) raises the Tavern spells' bonus (`auras.spell_bonus_atk` /
//! `spell_bonus_hp`, as `Intrepid Botanist`'s) by +1/+1 for the rest of the game.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Unit};

pub const ID: CardId = 121041;

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
        ctx.auras.spell_bonus_atk += 1;
        ctx.auras.spell_bonus_hp += 1;
    }
}
