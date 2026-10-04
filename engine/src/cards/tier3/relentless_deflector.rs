//! `Relentless Deflector` (`BG34_405`) — Tier 3 Mech (`5/4`).
//! Has **Taunt** while this has **Divine Shield**. **Avenge (3):** Gain **Divine Shield**.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 327;
pub const NAME: &str = "Relentless Deflector";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 5, 4, 3)
        .with_tribe(Tribe::Mech)
        .on_friendly_death(|ctx, self_idx, _| on_friendly_death(&mut ctx.board[self_idx]))
}

pub fn sync_taunt(unit: &mut Unit) {
    if unit.card_id == ID {
        unit.taunt = unit.divine_shield;
    }
}

/// Avenge (3): gain Divine Shield (and with it Taunt). Counts Tavern deaths too.
pub fn on_friendly_death(unit: &mut Unit) {
    if unit.avenge(3) {
        unit.apply_keyword(Keyword::DivineShield, false);
        unit.taunt = true;
    }
}
