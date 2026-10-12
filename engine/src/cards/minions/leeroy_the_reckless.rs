//! `Leeroy the Reckless` (`BG23_318`) — Tier 5 Neutral (`6/2`).
//!
//! Deathrattle: Destroy the minion that killed this.
//!
//! The killer is destroyed even if it is friendly (e.g. your own `Boom-in-a-Box` at Start of
//! Combat).

use crate::cards::{BoardCtx, CardTemplate};
use crate::model::{CardId, Unit};

pub const ID: CardId = 529;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Leeroy the Reckless", 6, 2, 5).on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut BoardCtx<'_>) {
    if let Some(killer) = dying.killed_by {
        ctx.destroy(killer);
    }
}
