//! `Baleful Incense` (`BG32_MagicItem_360`) — Lesser, 3 Gold, Undead.
//!
//! Start of Combat: Give your left and right-most Undead Reborn.
//!
//! Step 1c (`docs/trinkets.md` §7.2): the left-most Undead, then the right-most, gain Reborn; a
//! single Undead gains it once.

use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 120137;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_start_of_combat(start_of_combat),
    )]
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let undead: Vec<usize> = (0..ctx.own.board.len())
        .filter(|&i| ctx.own.board[i].tribe.matches(Tribe::Undead))
        .collect();
    for i in super::ends(&undead) {
        ctx.own.board[i].apply_keyword(Keyword::Reborn, false);
    }
}
