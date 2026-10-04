//! `De-volition-ist` (`BG36_102`) — Tier 5 Aberration (`4/8`).
//!
//! After this attacks, deal damage equal to (`double` if Golden) this minion's Attack to the highest-Health enemy minion.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 508;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "De-volition-ist", 4, 8, 5).with_tribe(Tribe::Aberration)
}

pub fn after_attack_damage(attacker: &Unit) -> i32 {
    if attacker.card_id != ID {
        return 0;
    }
    let mult = if attacker.is_golden { 2 } else { 1 };
    (attacker.attack * mult).max(0)
}
