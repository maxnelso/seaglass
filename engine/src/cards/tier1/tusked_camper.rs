//! `Tusked Camper` (`BG33_886`) — Tier 1 Quilboar (`2/3`).
//! **Rally:** This plays a **Blood Gem** (`2` if Golden) on itself.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 117;
pub const NAME: &str = "Tusked Camper";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 3, 1).with_tribe(Tribe::Quilboar)
}

pub fn on_rally(attacker: &mut Unit) -> Vec<Unit> {
    let mult = if attacker.is_golden { 2 } else { 1 };
    attacker.add_stats(mult, mult);
    Vec::new()
}
