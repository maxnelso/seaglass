//! `Stalwart Kodo` (`BG34_322`) — Tier 7 Beast (`15/30`).
//!
//! After you summon a different minion in combat, give it double (`triple` if Golden) this
//! minion's maximum stats. (`3` times per combat.)
//!
//! Another `Stalwart Kodo` summoned in combat doesn't trigger this. With fewer triggers left than
//! minions summoned at once (`Cadaver Caretaker`'s Skeletons), the first summoned, the left-most,
//! get it.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 709;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Stalwart Kodo", 15, 30, 7)
        .with_tribe(Tribe::Beast)
        .on_reset_turn_charges(reset_charges)
        .on_combat_start(reset_charges)
        .on_friendly_summon(on_friendly_summon)
}

/// Three summon triggers per combat.
pub fn reset_charges(unit: &mut Unit) {
    unit.charges = 3;
}

pub fn on_friendly_summon(unit: &mut Unit, summoned: &mut Unit, in_combat: bool) {
    if !in_combat || unit.charges == 0 || summoned.card_id == ID {
        return;
    }
    unit.charges -= 1;
    let mult = if unit.is_golden { 3 } else { 2 };
    summoned.add_stats(unit.max_attack.max(0) * mult, unit.max_health.max(0) * mult);
}
