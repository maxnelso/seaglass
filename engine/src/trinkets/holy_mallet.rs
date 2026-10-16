//! `Holy Mallet` (`BG30_MagicItem_902`) — Lesser, 0 Gold, Dragon.
//!
//! Start of Combat: Give your left and right-most minions Divine Shield.
//!
//! Step 1c (`docs/trinkets.md` §7.2): the left-most minion, then the right-most, gain Divine
//! Shield; a single minion gains it once.

use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::{CardId, Keyword};

pub const ID: CardId = 111401;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_start_of_combat(start_of_combat),
    )]
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let all: Vec<usize> = (0..ctx.own.board.len()).collect();
    for i in super::ends(&all) {
        ctx.own.board[i].apply_keyword(Keyword::DivineShield, false);
    }
}
