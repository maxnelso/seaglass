//! `Resistance` (`BG36_MidGameEffect_000t7`) — turns 7–10, weight 100.
//!
//! Start of Combat: Double this minion's Health.
//!
//! Start of Combat: it gains its Health. Not on Beasts, Undead, or minions with no type; not in a
//! Dragon lobby (Hotfix 36.2.1; `docs/dark_gifts.md` §5). A stat gift.

use crate::cards::{BoardCtx, CardFlags, CardHooks, CardTemplate};
use crate::dark_gifts::OfferCtx;
use crate::model::Tribe;
use crate::model::{CardId, UnitId};
pub const ID: CardId = 132205;

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
    let health = ctx.board[pos].health.max(0);
    ctx.buff_unit(pos, 0, health, "Resistance");
}

fn offerable(ctx: &OfferCtx<'_>, t: &CardTemplate) -> bool {
    super::has_type(t)
        && !t.tribe.matches(Tribe::Beast)
        && !t.tribe.matches(Tribe::Undead)
        && !ctx.lobby_has(Tribe::Dragon)
}
