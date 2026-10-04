//! `Drakkari Enchanter` (`BG26_ICC_901`) — Tier 5 Neutral (`1/5`).
//!
//! Your end of turn effects trigger twice (`three times` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Unit};

pub const ID: CardId = 512;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Drakkari Enchanter", 1, 5, 5)
}

pub fn end_of_turn_multiplier(board: &[Unit]) -> u32 {
    let mut mult = 1u32;
    for u in board {
        if u.card_id == ID {
            mult = mult.max(if u.is_golden { 3 } else { 2 });
        }
    }
    mult
}
