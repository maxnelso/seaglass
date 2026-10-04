//! `Stalwart Kodo` (`BG34_322`) — Tier 7 Beast (`16/32`).
//!
//! After you summon a minion in combat, give it this minion's maximum stats (double if Golden).
//! (`3` times per combat.)

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 709;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Stalwart Kodo", 16, 32, 7)
        .with_tribe(Tribe::Beast)
        .on_reset_turn_charges(reset_charges)
        .on_combat_start(reset_charges)
        .on_friendly_summon(on_friendly_summon)
}

/// Three summon triggers per combat.
pub fn reset_charges(unit: &mut Unit) {
    unit.kodo_triggers_left = 3;
}

pub fn on_friendly_summon(unit: &mut Unit, summoned: &mut Unit, in_combat: bool) {
    if !in_combat || unit.kodo_triggers_left == 0 {
        return;
    }
    unit.kodo_triggers_left -= 1;
    let mult = if unit.is_golden { 2 } else { 1 };
    summoned.add_stats(unit.max_attack.max(0) * mult, unit.max_health.max(0) * mult);
}
