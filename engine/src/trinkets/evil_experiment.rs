//! `Evil Experiment` (`BG36_MagicItem_416`) — Greater, 4 Gold.
//!
//! After your Deity awakens, give it Reborn.
//!
//! `trinket_deity_awakened`: the Deity that just awakened in combat gains Reborn.

use crate::cards::{BoardCtx, CardHooks};
use crate::model::{CardId, Keyword};

pub const ID: CardId = 134757;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_deity_awakened(deity_awakened),
    )]
}

fn deity_awakened(ctx: &mut BoardCtx<'_>, idx: usize) {
    if let Some(deity) = ctx.board.get_mut(idx) {
        deity.apply_keyword(Keyword::Reborn, false);
    }
}
