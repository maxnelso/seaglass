//! `Eyes of the Earth Mother` — Tier 6 Tavern spell (`4` Gold).
//!
//! Make a friendly minion of Tier 4 or lower Golden.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Eyes of the Earth Mother`: make a friendly minion of Tier 4 or lower Golden.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, _: &mut Rng) {
    if board_pos < state.board.len()
        && state.board[board_pos].tavern_tier <= 4
        && !state.board[board_pos].is_golden
    {
        state.board[board_pos].make_golden();
        state.sync_all_auras();
    }
}
