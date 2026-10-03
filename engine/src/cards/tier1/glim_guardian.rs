//! `Glim Guardian` (`BG29_888`) — Tier 1 Dragon (`1/4`).
//! **Rally:** Gain `+2` (`+4` if Golden) Attack.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 106;
pub const NAME: &str = "Glim Guardian";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 4, 1).with_tribe(Tribe::Dragon)
}

pub fn on_rally(attacker: &mut Unit) -> Vec<Unit> {
    let bonus = if attacker.is_golden { 4 } else { 2 };
    attacker.add_stats(bonus, 0);
    Vec::new()
}
