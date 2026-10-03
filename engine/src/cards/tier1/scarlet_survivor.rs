//! `Scarlet Survivor` (`BG35_814`) — Tier 1 Dragon (`3/3`).
//! Once this reaches `6` Attack, gain **Divine Shield**.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 107;
pub const NAME: &str = "Scarlet Survivor";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 1).with_tribe(Tribe::Dragon)
}

pub fn check_threshold(unit: &mut Unit) {
    if unit.card_id == ID && !unit.threshold_triggered && unit.attack >= 6 {
        unit.divine_shield = true;
        unit.threshold_triggered = true;
    }
}
