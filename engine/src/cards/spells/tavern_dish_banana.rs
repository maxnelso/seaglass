//! `Tavern Dish Banana` — Tier 1 Tavern spell (`1` Gold).
//!
//! Give a minion +2/+2.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Tavern Dish Banana`: give a minion +2/+2.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, _: &mut Rng) {
    if board_pos < state.board.len() {
        let (atk, hp) = state.auras.spell_stat_buff(2, 2);
        state.board[board_pos].add_stats(atk, hp);
    }
}
