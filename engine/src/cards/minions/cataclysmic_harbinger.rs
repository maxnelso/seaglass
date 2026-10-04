//! `Cataclysmic Harbinger` (`BG35_123`) — Tier 5 Neutral (`6/10`).
//!
//! At the end of your turn, get a copy (`2` copies if Golden) of the last Tavern spell you cast.

use crate::cards::{spells, CardTemplate};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 505;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Cataclysmic Harbinger", 6, 10, 5).on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let Some(last_spell_id) = state.auras.last_tavern_spell_cast else {
        return;
    };
    let count = if state.board[self_idx].is_golden {
        2
    } else {
        1
    };
    for _ in 0..count {
        if let Some(spell) = spells::spell_by_id(last_spell_id) {
            state.add_to_hand(spell);
        }
    }
}
