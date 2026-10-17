//! `Transcendence` (`BG36_MidGameEffect_000t81`) — turns 8–, weight 200.
//!
//! Start of Combat: Triple this minion's stats.
//!
//! Start of Combat: it gains twice its Attack and Health. Only on minions with no type; not in a
//! Dragon lobby (Hotfix 36.2.1; `docs/dark_gifts.md` §5). A stat gift.

use crate::cards::{BoardCtx, CardFlags, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::Tribe;
use crate::model::{CardId, UnitId};
pub const ID: CardId = 133482;

/// The gift's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::GIFT_GIVES_STATS)
            .on_gift_start_of_combat(start_of_combat)
            .on_gift_offerable(offerable),
    )]
}

fn start_of_combat(ctx: &mut BoardCtx<'_>, id: UnitId, _: usize) {
    let Some(pos) = ctx.board.iter().position(|u| u.id == id && u.health > 0) else {
        return;
    };
    let (attack, health) = (ctx.board[pos].attack.max(0), ctx.board[pos].health.max(0));
    ctx.buff_unit(
        pos,
        attack.saturating_mul(2),
        health.saturating_mul(2),
        "Transcendence",
    );
}

fn offerable(ctx: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    !super::has_type(t) && !ctx.lobby_has(Tribe::Dragon)
}
