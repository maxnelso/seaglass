//! `Repair Job` — Tier 3 Tavern spell (`2` Gold).
//!
//! Give a minion +4/+8.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Repair Job`: give a minion +4/+8.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, _: &mut Rng) {
    if board_pos < state.board.len() {
        let (atk, hp) = state.auras.spell_stat_buff(4, 8);
        state.board[board_pos].add_stats(atk, hp);
    }
}
