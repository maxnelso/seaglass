//! `Relentless Deflector` (`BG34_405`) — Tier 3 Mech (`5/4`).
//! Has **Taunt** while this has **Divine Shield**. **Avenge (3):** Gain **Divine Shield**.

use crate::cards::{BoardCtx, CardTemplate};
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 327;
pub const NAME: &str = "Relentless Deflector";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 5, 4, 3)
        .with_tribe(Tribe::Mech)
        .on_friendly_death(on_friendly_death)
        .on_sync_aura(|u, _| sync_taunt(u))
}

/// Has Taunt while this has Divine Shield.
pub fn sync_taunt(unit: &mut Unit) {
    unit.taunt = unit.divine_shield;
}

/// Avenge (3): gain Divine Shield (and with it Taunt).
pub fn on_friendly_death(ctx: &mut BoardCtx<'_>, self_idx: usize, _dying: &Unit) {
    if !ctx.in_combat || !ctx.board[self_idx].avenge(3) {
        return;
    }
    let unit = &mut ctx.board[self_idx];
    unit.apply_keyword(Keyword::DivineShield, false);
    unit.taunt = true;
}
