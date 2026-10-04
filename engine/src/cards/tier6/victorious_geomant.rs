//! `Victorious Geomant` (`BG36_370`) — Tier 6 Quilboar (`10/10`).
//!
//! Activate (2): Get 6 (`12` if Golden) Blood Gems. Cast any that don't fit on your left-most minion.

use crate::cards::{self, tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 631;
pub const ACTIVATE_COST: u32 = 2;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Victorious Geomant", 10, 10, 6)
        .with_tribe(Tribe::Quilboar)
        .with_activate_cost(ACTIVATE_COST)
}

pub fn on_activate(state: &mut TavernState, source_pos: usize, rng: &mut Rng) {
    if source_pos >= state.board.len() {
        return;
    }
    let count = if state.board[source_pos].is_golden {
        12
    } else {
        6
    };
    let mut overflow = 0u32;
    for _ in 0..count {
        if state.hand.len() < 10 {
            state.add_to_hand(tokens::make_blood_gem());
        } else {
            overflow += 1;
        }
    }
    if overflow > 0 && !state.board.is_empty() {
        state.board[0].play_blood_gems(overflow, &state.auras);
        cards::resolve_roogug_procs(&mut state.board, &state.auras, rng);
    }
}
