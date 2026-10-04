//! `Gatekeeper Amalgam` (`BG36_640`) — Tier 6 All (`6/6`).
//!
//! Whenever you cast a spell on this, it casts `Misplaced Tea Set` (`twice` if Golden).

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 612;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Gatekeeper Amalgam", 6, 6, 6).with_tribe(Tribe::All)
}

pub fn after_cast_targeted_spell(state: &mut TavernState, target_pos: usize, rng: &mut Rng) {
    if target_pos >= state.board.len() || state.board[target_pos].card_id != ID {
        return;
    }
    let count = if state.board[target_pos].is_golden {
        2
    } else {
        1
    };
    for _ in 0..count {
        spells::apply_misplaced_tea_set(state, rng);
    }
}
