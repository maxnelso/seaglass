//! `Glambot` (`BG36_853`) — Tier 4 Mech (`4/4`).
//!
//! Whenever you cast a spell on a Mech, Magnetize a 4/4 Satellite to it (`twice` if Golden).

use crate::cards::{self, tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 424;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Glambot", 4, 4, 4).with_tribe(Tribe::Mech)
}

pub fn after_cast_targeted_spell(state: &mut TavernState, target_pos: usize) {
    if target_pos >= state.board.len() || !state.board[target_pos].tribe.matches(Tribe::Mech) {
        return;
    }
    let mut sat_count = 0u32;
    for u in &state.board {
        if u.card_id == ID {
            sat_count += if u.is_golden { 2 } else { 1 };
        }
    }
    for _ in 0..sat_count {
        let sat = tokens::make_satellite(false);
        let repeats = 1 + state.board[target_pos].extra_magnetize_this_turn;
        state.board[target_pos].extra_magnetize_this_turn = 0;
        for _ in 0..repeats {
            state.board[target_pos].add_stats(sat.attack, sat.health);
            cards::on_magnetize_transfer(&sat, &mut state.board[target_pos]);
            cards::after_play_minion(state, sat.card_id, sat.tribe, target_pos, true);
        }
    }
}
