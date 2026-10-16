//! `Beetle Band` (`BG32_MagicItem_860`) — Lesser, 0 Gold, Beast.
//!
//! Avenge (5): Summon a 2/2 Beetle. Give it Taunt.
//!
//! `trinket_friendly_death`: every 5th friendly minion that dies in a combat (`counter`, which
//! starts again each combat: confirmed) summons a `Beetle` (2/2 plus
//! the player's Beetle bonus, `tokens::make_beetle`) with Taunt at the right end, if there is
//! room.

use crate::cards::{tokens, BoardCtx, CardHooks};
use crate::heroes::HERO_SOURCE;
use crate::model::{CardId, Keyword, Unit};

pub const ID: CardId = 120760;

/// Friendly deaths per Avenge.
const AVENGE: i32 = 5;

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
        summon_beetles(ctx, 1, "Beetle Band");
    }
}

/// Summon `count` Beetles with Taunt at the right end (the Beetle Bands).
pub fn summon_beetles(ctx: &mut BoardCtx<'_>, count: usize, reason: &'static str) {
    for _ in 0..count {
        let mut beetle = tokens::make_beetle(false, ctx.auras);
        beetle.apply_keyword(Keyword::Taunt, false);
        ctx.cursor = ctx.board.len();
        ctx.summon_as(HERO_SOURCE, beetle, reason);
    }
}
