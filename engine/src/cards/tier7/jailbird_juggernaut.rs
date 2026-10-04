//! `Jailbird Juggernaut` (`BG36_333`) — Tier 7 Quilboar (`6/15`).
//!
//! Rally: Summon a Golem with this minion's stats (double if Golden) to attack the target first.

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 705;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Jailbird Juggernaut", 6, 15, 7).with_tribe(Tribe::Quilboar)
}

pub fn on_rally(attacker: &Unit) -> Vec<Unit> {
    let mult = if attacker.is_golden { 2 } else { 1 };
    let atk = attacker.attack.max(0) * mult;
    let hp = attacker.health.max(1) * mult;
    vec![tokens::make_blood_golem(atk, hp, attacker.is_golden)]
}
