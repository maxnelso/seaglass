//! `Snazzy Phantom` (`BG36_515`) — Tier 6 Undead (`6/8`).
//!
//! After a friendly minion is Reborn, give stats equal to (`double` if Golden) its Attack to your right-most Undead.

use crate::cards::{BoardCtx, CardTemplate};
use crate::model::{CardId, Tribe};

pub const ID: CardId = 622;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Snazzy Phantom", 6, 8, 6)
        .with_tribe(Tribe::Undead)
        .on_after_friendly_reborn(after_friendly_reborn)
}

pub fn after_friendly_reborn(ctx: &mut BoardCtx<'_>, self_idx: usize, reborn_attack: i32) {
    let atk = reborn_attack.max(0);
    if atk == 0 {
        return;
    }
    let buff = atk * (if ctx.board[self_idx].is_golden { 2 } else { 1 });
    if let Some(target) = ctx
        .board
        .iter()
        .rposition(|u| u.health > 0 && u.tribe.matches(Tribe::Undead))
    {
        ctx.buff_unit(target, buff, buff, "Snazzy Phantom");
    }
}
