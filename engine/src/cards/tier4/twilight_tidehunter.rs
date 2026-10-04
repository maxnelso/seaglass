//! `Twilight Tidehunter` (`BG36_703`) — Tier 4 Murloc (`4/6`).
//!
//! Whenever you cast a spell on this, give the left-most minion in your hand `+8/+8` (`+16/+16` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 458;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Twilight Tidehunter", 4, 6, 4).with_tribe(Tribe::Murloc)
}

pub fn after_cast_targeted_spell(state: &mut TavernState, target_pos: usize) {
    if target_pos >= state.board.len() || state.board[target_pos].card_id != ID {
        return;
    }
    let buff = if state.board[target_pos].is_golden { 16 } else { 8 };
    if let Some(h) = state.hand.iter_mut().find(|u| !u.is_spell) {
        h.add_stats(buff, buff);
    }
}
