//! `Banana Slamma` (`BG26_802`) — Tier 4 Beast (`3/6`).
//!
//! After you summon a Beast in combat, double (`triple` if Golden) its Attack.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 403;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Banana Slamma", 3, 6, 4)
        .with_tribe(Tribe::Beast)
        .on_friendly_summon(on_friendly_summon)
}

pub fn on_friendly_summon(unit: &mut Unit, summoned: &mut Unit, in_combat: bool) {
    if !in_combat || !summoned.tribe.matches(Tribe::Beast) {
        return;
    }
    let extra = if unit.is_golden {
        summoned.attack * 2
    } else {
        summoned.attack
    };
    summoned.add_stats(extra, 0);
}
