//! `Balinda Stonehearth` (`BG35_883`) — Tier 6 Neutral (`6/6`).
//!
//! Your spells that target friendly minions cast twice (`three times` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Unit};

pub const ID: CardId = 602;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Balinda Stonehearth", 6, 6, 6)
}

pub fn targeted_spell_multiplier(board: &[Unit]) -> u32 {
    let mut mult = 1u32;
    for u in board {
        if u.card_id == ID {
            mult = mult.max(if u.is_golden { 3 } else { 2 });
        }
    }
    mult
}
