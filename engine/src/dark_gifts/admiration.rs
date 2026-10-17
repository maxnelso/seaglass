//! `Admiration` (`BG36_MidGameEffect_000t9`) — turns 7–, weight 100.
//!
//! Start of Combat: Gain the Attack of the minion to the left.
//!
//! Start of Combat: it gains the Attack of the friendly minion to its left as Start of Combat
//! reaches it; nothing for the left-most minion. A stat gift (`docs/dark_gifts.md` §11.1).

use crate::cards::{BoardCtx, CardFlags, CardHooks};
use crate::model::{CardId, UnitId};

pub const ID: CardId = 132207;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_GIVES_STATS)
            .on_gift_start_of_combat(start_of_combat),
    )]
}

fn start_of_combat(ctx: &mut BoardCtx<'_>, id: UnitId, _: usize) {
    let Some(pos) = ctx.board.iter().position(|u| u.id == id && u.health > 0) else {
        return;
    };
    if pos == 0 {
        return;
    }
    let attack = ctx.board[pos - 1].attack.max(0);
    ctx.buff_unit(pos, attack, 0, "Admiration");
}
