//! `Eternal Tycoon` (`BG34_403`) — Tier 5 Undead (`4/8`).
//!
//! Avenge (5): Summon an `Eternal Knight` (or a Golden `Eternal Knight` if Golden). It attacks immediately.

use crate::cards::{tier2, BoardCtx, CardTemplate};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 517;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Eternal Tycoon", 4, 8, 5)
        .with_tribe(Tribe::Undead)
        .on_friendly_death(on_friendly_death)
}

/// Avenge (5), combat deaths only: the Knight is summoned to this minion's right.
pub fn on_friendly_death(ctx: &mut BoardCtx<'_>, self_idx: usize, _dying: &Unit) {
    if !ctx.in_combat || !ctx.board[self_idx].avenge(5) {
        return;
    }
    let (src_id, is_golden) = (ctx.board[self_idx].id, ctx.board[self_idx].is_golden);
    let mut knight = tier2::eternal_knight::template().instantiate();
    if is_golden {
        knight.make_golden();
    }
    ctx.cursor = self_idx + 1;
    if let Some(pos) = ctx.summon_as(src_id, knight, "Eternal Tycoon") {
        let knight_id = ctx.board[pos].id;
        ctx.pending_attacks.push(knight_id);
    }
}
