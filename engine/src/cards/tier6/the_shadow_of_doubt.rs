//! `The Shadow of Doubt` (`BG36_109`) — Tier 6 Aberration (`6/8`).
//!
//! Whenever a card is added to your hand, give your Deity `+4/+5` (`+8/+10` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 623;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "The Shadow of Doubt", 6, 8, 6).with_tribe(Tribe::Aberration)
}

pub fn on_card_added_to_hand(board: &[Unit], auras: &mut PlayerAuras) {
    let mut mult = 0i32;
    for u in board {
        if u.health > 0 && u.card_id == ID {
            mult += if u.is_golden { 2 } else { 1 };
        }
    }
    if mult > 0 {
        auras.deity.attack += 4 * mult;
        auras.deity.health += 5 * mult;
    }
}
