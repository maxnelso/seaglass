//! `Lurking Leviathan` (`BG35_602`) — Tier 5 Beast (`3/9`).
//!
//! Whenever you summon a Beast, give it `+3` (`+6` if Golden) Attack and improve this permanently.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 532;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Lurking Leviathan", 3, 9, 5)
        .with_tribe(Tribe::Beast)
        .on_friendly_summon(on_friendly_summon)
}

pub fn on_friendly_summon(unit: &mut Unit, summoned: &mut Unit, _in_combat: bool) {
    if !summoned.tribe.matches(Tribe::Beast) {
        return;
    }
    let base = if unit.is_golden { 6 } else { 3 };
    summoned.add_stats(base * (1 + unit.stacks), 0);
    unit.stacks += 1;
}
