//! `Doppelganger` (`BG36_MidGameEffect_000t90`) — turns 6–8, weight 100.
//!
//! Start of Combat: Summon a copy of this.
//!
//! Start of Combat: summon a copy of it as it is (its gifts too) to its right, if there is room;
//! the copy's own Start of Combat doesn't trigger (it wasn't listed). No offer rule of its own
//! (confirmed; `docs/dark_gifts.md` §5.2).

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, UnitId};

pub const ID: CardId = 134733;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_gift_start_of_combat(start_of_combat),
    )]
}

fn start_of_combat(ctx: &mut BoardCtx<'_>, id: UnitId, _: usize) {
    let Some(pos) = ctx.board.iter().position(|u| u.id == id && u.health > 0) else {
        return;
    };
    let copy = ctx.board[pos].clone();
    ctx.cursor = pos + 1;
    ctx.summon_as(id, copy, "Doppelganger");
}
