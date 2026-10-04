//! `Tricky Trousers` — Tier 3 Tavern spell (`1` Gold).
//!
//! Give a minion +1/+2 and toggle its Taunt.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Tricky Trousers`: give a minion +1/+2 and toggle its Taunt.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, _: &mut Rng) {
    if board_pos < state.board.len() {
        let (atk, hp) = state.auras.spell_stat_buff(1, 2);
        state.board[board_pos].add_stats(atk, hp);
        state.board[board_pos].taunt = !state.board[board_pos].taunt;
    }
}
