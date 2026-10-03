//! `Treasure Parrot` (`BG36_763`) — Tier 3 Beast/Pirate (`5/5`).
//! Once this deals 35 damage, get a (`two` if Golden) `Golden Touch`(es).

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 337;
pub const NAME: &str = "Treasure Parrot";
pub const DAMAGE_THRESHOLD: i32 = 35;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 5, 5, 3).with_tribe(Tribe::BeastPirate)
}

pub fn on_dealt_damage(
    unit: &mut Unit,
    amount: i32,
    hand: &mut Vec<Unit>,
    hand_summoned: &mut Vec<bool>,
) {
    if unit.card_id != ID || amount <= 0 {
        return;
    }
    unit.damage_dealt_counter += amount;
    if !unit.threshold_triggered && unit.damage_dealt_counter >= DAMAGE_THRESHOLD {
        unit.threshold_triggered = true;
        let count = if unit.is_golden { 2 } else { 1 };
        for _ in 0..count {
            if hand.len() < 10 {
                hand.push(tokens::make_golden_touch());
                hand_summoned.push(false);
            }
        }
    }
}
