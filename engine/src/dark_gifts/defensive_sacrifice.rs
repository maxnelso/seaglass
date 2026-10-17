//! `Defensive Sacrifice` (`BG36_MidGameEffect_000t2`) — turns 6–9, weight 100.
//!
//! +10 Health. Deathrattle: Give this minion's max Health to another friendly minion.
//!
//! Given: +10 Health. Deathrattle: a random other living friendly minion (one draw) gains its
//! maximum Health (`max_health`); nothing without one. Only on minions with a type
//! (`docs/dark_gifts.md` §5.2). A stat gift.

use crate::cards::{BoardCtx, CardFlags, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, DarkGift, Unit};

pub const ID: CardId = 132200;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_GIVES_STATS)
            .on_gift_given(given)
            .on_gift_deathrattle(deathrattle)
            .on_gift_offerable(offerable),
    )]
}

fn given(unit: &mut Unit, _: &mut DarkGift) {
    unit.add_stats(0, 10);
}

fn deathrattle(dying: &Unit, _: &DarkGift, ctx: &mut BoardCtx<'_>) {
    let others: Vec<usize> = (0..ctx.board.len())
        .filter(|&i| ctx.board[i].health > 0 && ctx.board[i].id != dying.id)
        .collect();
    if others.is_empty() {
        return;
    }
    let pos = others[ctx.rng.below(others.len())];
    let amount = dying.max_health.max(0);
    ctx.buff_unit(pos, 0, amount, "Defensive Sacrifice");
}

fn offerable(_: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    super::has_type(t)
}
