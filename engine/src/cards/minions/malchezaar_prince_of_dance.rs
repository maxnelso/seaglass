//! `Malchezaar, Prince of Dance` (`BG26_524`) — Tier 3 Demon (`4/3`).
//! Two (`Four` if Golden) **Refreshes** each turn cost Health instead of Gold.

use crate::cards::{CardFlags, CardTemplate};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 323;
pub const NAME: &str = "Malchezaar, Prince of Dance";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 3, 3)
        .with_tribe(Tribe::Demon)
        .with_flags(CardFlags::HEALTH_REFRESHES)
        .on_reset_turn_charges(reset_turn_charges)
        .on_made_golden(|u| u.charges += 2)
}

pub fn reset_turn_charges(unit: &mut Unit) {
    unit.charges = if unit.is_golden { 4 } else { 2 };
}
