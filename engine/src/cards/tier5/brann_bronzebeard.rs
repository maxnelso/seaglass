//! `Brann Bronzebeard` (`BG_LOE_077`) — Tier 5 Neutral (`2/4`).
//!
//! Your Battlecries trigger twice (`three times` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Unit};

pub const ID: CardId = 504;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Brann Bronzebeard", 2, 4, 5)
}

pub fn battlecry_multiplier(board: &[Unit]) -> u32 {
    let mut mult = 1u32;
    for u in board {
        if u.card_id == ID {
            mult = mult.max(if u.is_golden { 3 } else { 2 });
        }
    }
    mult
}
