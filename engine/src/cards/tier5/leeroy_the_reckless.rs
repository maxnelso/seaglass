//! `Leeroy the Reckless` (`BG23_318`) — Tier 5 Neutral (`6/2`).
//!
//! Deathrattle: Destroy the minion that killed this.

use crate::cards::{BoardCtx, CardTemplate};
use crate::model::{CardId, Unit};

pub const ID: CardId = 529;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Leeroy the Reckless", 6, 2, 5).on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut BoardCtx<'_>) {
    if let Some(killer) = dying.killed_by {
        ctx.destroy_enemy(killer);
    }
}
