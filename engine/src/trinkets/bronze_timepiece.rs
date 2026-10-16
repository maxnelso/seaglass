//! `Bronze Timepiece` (`BG30_MagicItem_995`) — Greater, 5 Gold, Dragon.
//!
//! Start of Combat: Give each of your minions Health equal to half its Attack.
//!
//! Step 1c (`docs/trinkets.md` §7.2): each friendly minion, left to right, gains Health equal to
//! half its Attack, rounded down (**engine decision**).

use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::CardId;

pub const ID: CardId = 112411;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_start_of_combat(start_of_combat),
    )]
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    for i in 0..ctx.own.board.len() {
        let half = ctx.own.board[i].attack.max(0) / 2;
        if half > 0 {
            ctx.own.buff_unit(i, 0, half, "Bronze Timepiece");
        }
    }
}
