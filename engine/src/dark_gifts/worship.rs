//! `Worship` (`BG36_MidGameEffect_000t89`) — turns 12–, weight 50.
//!
//! Start of Combat: Gain the stats of your Deity.
//!
//! Start of Combat: it gains the Attack and Health of the player's Deity (`auras.deity`). Only for
//! a player with a Deity (confirmed; `docs/dark_gifts.md` §5.2). A stat gift.

use crate::cards::{BoardCtx, CardFlags, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::{CardId, DeityKind, UnitId};

pub const ID: CardId = 134732;

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
    let (attack, health) = (ctx.auras.deity.attack, ctx.auras.deity.health);
    ctx.buff_unit(pos, attack, health, "Worship");
}

fn offerable(ctx: &OfferCtx<'_>, _: &CardTemplate) -> bool {
    ctx.auras.deity.kind != DeityKind::None
}
