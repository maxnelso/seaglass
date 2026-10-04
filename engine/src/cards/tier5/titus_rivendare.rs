//! `Titus Rivendare` (`BG25_354`) — Tier 5 Neutral (`1/7`).
//!
//! Your Deathrattles trigger an extra time (`2` extra times if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Unit};

pub const ID: CardId = 551;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Titus Rivendare", 1, 7, 5)
}

pub fn extra_deathrattle_triggers(board: &[Unit]) -> u32 {
    let mut extra = 0u32;
    for u in board {
        if u.card_id == ID && u.health > 0 {
            extra += if u.is_golden { 2 } else { 1 };
        }
    }
    extra
}
