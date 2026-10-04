//! `Shamanic Tidecaller` (`BG36_704`) — Tier 5 Murloc (`5/7`).
//!
//! Whenever you cast a spell on a Murloc, give Murlocs in your hand and board `+3/+3` (`+6/+6` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 546;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Shamanic Tidecaller", 5, 7, 5).with_tribe(Tribe::Murloc)
}

pub fn after_cast_targeted_spell(state: &mut TavernState, target_pos: usize) {
    if target_pos >= state.board.len() || !state.board[target_pos].tribe.matches(Tribe::Murloc) {
        return;
    }
    let mut total_buff = 0i32;
    for u in &state.board {
        if u.card_id == ID {
            total_buff += if u.is_golden { 6 } else { 3 };
        }
    }
    if total_buff > 0 {
        for u in &mut state.board {
            if u.tribe.matches(Tribe::Murloc) {
                u.add_stats(total_buff, total_buff);
            }
        }
        for h in &mut state.hand {
            if !h.is_spell && h.tribe.matches(Tribe::Murloc) {
                h.add_stats(total_buff, total_buff);
            }
        }
    }
}
