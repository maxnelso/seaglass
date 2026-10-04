//! `Drustfallen Butcher` (`BG32_324`) — Tier 5 Undead (`2/7`).
//!
//! Avenge (4): Get a (`2` if Golden) `Butchering`(s).

use crate::cards::{spells, BoardCtx, CardTemplate};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 513;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Drustfallen Butcher", 2, 7, 5)
        .with_tribe(Tribe::Undead)
        .on_friendly_death(on_friendly_death)
}

/// Avenge (4), combat deaths only.
pub fn on_friendly_death(ctx: &mut BoardCtx<'_>, self_idx: usize, _dying: &Unit) {
    if !ctx.in_combat || !ctx.board[self_idx].avenge(4) {
        return;
    }
    for _ in 0..ctx.board[self_idx].golden_mult() {
        if let Some(spell) = spells::spell_by_id(spells::SPELL_BUTCHERING) {
            ctx.add_to_hand(spell);
        }
    }
}
