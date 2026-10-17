//! `Jaws of Death` (`BG36_MidGameEffect_000t16`) — turns 5–, weight 200.
//!
//! Start of Combat: Trigger this minion's Deathrattles.
//!
//! Start of Combat: its Deathrattles (its card's, the ones an effect gave it, its gifts') trigger
//! as `Deathstrider` makes them, repeated by `Titus Rivendare` and the trinkets' extra
//! Deathrattles, summons to its right. Only on Deathrattle minions (`docs/dark_gifts.md` §5.2).

use crate::cards::{self, BoardCtx, CardHooks, CardTemplate, Passive};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, UnitId};

pub const ID: CardId = 132443;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_gift_start_of_combat(start_of_combat)
            .on_gift_offerable(offerable),
    )]
}

fn start_of_combat(ctx: &mut BoardCtx<'_>, id: UnitId, _: usize) {
    let Some(pos) = ctx.board.iter().position(|u| u.id == id && u.health > 0) else {
        return;
    };
    let unit = ctx.board[pos].clone();
    let repeats = 1
        + cards::board_passive(ctx.board, Passive::ExtraDeathrattles)
        + crate::trinkets::extra_deathrattles(ctx.auras, &unit);
    ctx.cursor = pos + 1;
    for _ in 0..repeats {
        cards::on_deathrattle(&unit, ctx);
    }
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    super::is_deathrattle(t)
}
