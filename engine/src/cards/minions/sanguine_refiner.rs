//! `Sanguine Refiner` (`BG33_885`) — Tier 5 Quilboar (`3/8`).
//!
//! Rally: Your Blood Gems give an extra `+1/+2` (`+2/+4` if Golden) this game.

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 543;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Sanguine Refiner", 3, 8, 5)
        .with_tribe(Tribe::Quilboar)
        .on_rally(|c| {
            on_rally(&c.board[c.attacker_pos], c.auras);
            Vec::new()
        })
}

pub fn on_rally(attacker: &Unit, auras: &mut PlayerAuras) {
    let mult = if attacker.is_golden { 2 } else { 1 };
    auras.blood_gem_bonus_atk += mult;
    auras.blood_gem_bonus_hp += 2 * mult;
}
