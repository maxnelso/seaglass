//! `Nightmare Corroder` (`BG36_115`) — Tier 4 Aberration (`4/5`).
//!
//! At the end of your turn, get a (`2` if Golden) Sludge Corrosion(s).

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 443;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Nightmare Corroder", 4, 5, 4).with_tribe(Tribe::Aberration)
}

pub fn on_end_turn(state: &mut TavernState) {
    let mut count = 0u32;
    for u in &state.board {
        if u.card_id == ID {
            count += if u.is_golden { 2 } else { 1 };
        }
    }
    for _ in 0..count {
        if state.hand.len() >= 10 {
            break;
        }
        state.add_to_hand(tokens::make_sludge_corrosion());
    }
}
