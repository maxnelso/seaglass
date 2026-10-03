//! `Malchezaar, Prince of Dance` (`BG26_524`) — Tier 3 Demon (`4/3`).
//! Two (`Four` if Golden) **Refreshes** each turn cost Health instead of Gold.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 323;
pub const NAME: &str = "Malchezaar, Prince of Dance";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 3, 3).with_tribe(Tribe::Demon)
}

pub fn reset_turn_charges(unit: &mut Unit) {
    if unit.card_id == ID {
        unit.malchezaar_refreshes_left = if unit.is_golden { 4 } else { 2 };
    }
}
