//! `Cataclysmic Harbinger` (`BG35_123`) — Tier 5 Neutral (`6/10`).
//!
//! At the end of your turn, get a copy (`2` copies if Golden) of the last Tavern spell you cast.

use crate::cards::{spells, CardTemplate};
use crate::model::CardId;
use crate::tavern::TavernState;

pub const ID: CardId = 505;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Cataclysmic Harbinger", 6, 10, 5)
}

pub fn on_end_turn(state: &mut TavernState) {
    let Some(last_spell_id) = state.auras.last_tavern_spell_cast else {
        return;
    };
    let mut count = 0u32;
    for u in &state.board {
        if u.card_id == ID {
            count += if u.is_golden { 2 } else { 1 };
        }
    }
    for _ in 0..count {
        if let Some(spell) = spells::spell_by_id(last_spell_id) {
            state.add_to_hand(spell);
        }
    }
}
