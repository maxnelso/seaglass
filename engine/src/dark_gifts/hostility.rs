//! `Hostility` (`BG36_MidGameEffect_000t71`) — turns 7–10, weight 100.
//!
//! +15 Attack. Start of Combat: Double this minion's Attack.
//!
//! Given: +15 Attack. Start of Combat: it gains its Attack. Not on Avenge minions, Venomous
//! minions, or minions with no type (`docs/dark_gifts.md` §5.2). A stat gift.

use crate::cards::{BoardCtx, CardFlags, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, DarkGift, Unit, UnitId};

pub const ID: CardId = 133359;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_GIVES_STATS)
            .on_gift_given(given)
            .on_gift_start_of_combat(start_of_combat)
            .on_gift_offerable(offerable),
    )]
}

fn given(unit: &mut Unit, _: &mut DarkGift) {
    unit.add_stats(15, 0);
}

fn start_of_combat(ctx: &mut BoardCtx<'_>, id: UnitId, _: usize) {
    let Some(pos) = ctx.board.iter().position(|u| u.id == id && u.health > 0) else {
        return;
    };
    let attack = ctx.board[pos].attack.max(0);
    ctx.buff_unit(pos, attack, 0, "Hostility");
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    super::has_type(t) && !super::is_avenge(t) && !t.venomous
}
