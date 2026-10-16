//! `Training Certificate` (`BG30_MagicItem_962`) — Lesser, 3 Gold, Dragon.
//!
//! Start of Combat: Double the stats of your two lowest-Attack minions.
//!
//! Step 1c (`docs/trinkets.md` §7.2): the two minions with the lowest Attack (among ties, the
//! left-most first: **engine decision**) gain their Attack and Health, the left one first.

use crate::cards::{CardHooks, HeroCombatCtx};
use crate::model::CardId;

pub const ID: CardId = 111417;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_start_of_combat(start_of_combat),
    )]
}

fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let board = &ctx.own.board;
    let mut by_attack: Vec<usize> = (0..board.len()).collect();
    by_attack.sort_by_key(|&i| (board[i].attack, i));
    let mut lowest: Vec<usize> = by_attack.into_iter().take(2).collect();
    lowest.sort_unstable();
    for i in lowest {
        let (attack, health) = (ctx.own.board[i].attack, ctx.own.board[i].health);
        ctx.own
            .buff_unit(i, attack.max(0), health.max(0), "Training Certificate");
    }
}
