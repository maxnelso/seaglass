//! `Barrier Banshee` (`BG36_514`) — Tier 5 Undead (`8/8`).
//!
//! After a friendly minion is Reborn, gain Divine Shield and `+8/+8` (`+16/+16` if Golden).

use crate::cards::{BoardCtx, CardTemplate};
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 502;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Barrier Banshee", 8, 8, 5)
        .with_tribe(Tribe::Undead)
        .on_after_friendly_reborn(after_friendly_reborn)
}

pub fn after_friendly_reborn(ctx: &mut BoardCtx<'_>, self_idx: usize, _reborn_attack: i32) {
    let buff = if ctx.board[self_idx].is_golden { 16 } else { 8 };
    ctx.board[self_idx].apply_keyword(Keyword::DivineShield, false);
    ctx.buff_unit(self_idx, buff, buff, "Barrier Banshee");
}
