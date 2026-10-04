//! `Parasitic Fleshling` (`BG36_114`) — Tier 4 Aberration (`4/6`).
//!
//! At the end of your turn, give your left-most minion `+2/+2` (`+4/+4` if Golden).
//! (Improved by each card you've discarded this game!)

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 444;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Parasitic Fleshling", 4, 6, 4).with_tribe(Tribe::Aberration)
}

pub fn on_end_turn(state: &mut TavernState) {
    if state.board.is_empty() {
        return;
    }
    let discarded = state.auras.cards_discarded as i32;
    let mut total_buff = 0i32;
    for u in &state.board {
        if u.card_id == ID {
            let mult = if u.is_golden { 2 } else { 1 };
            total_buff += (2 + discarded) * mult;
        }
    }
    if total_buff > 0 {
        state.board[0].add_stats(total_buff, total_buff);
    }
}
