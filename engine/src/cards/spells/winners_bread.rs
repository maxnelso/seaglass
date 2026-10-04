//! `Winner's Bread` — Tier 2 Tavern spell (`2` Gold).
//!
//! Give a minion +2/+3; if you win your next combat, it gets a Blood Gem next turn.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Winner's Bread`: give a minion +2/+3; if you win your next combat, it gets a Blood Gem next
/// turn.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, _: &mut Rng) {
    if board_pos < state.board.len() {
        let (atk, hp) = state.auras.spell_stat_buff(2, 3);
        state.board[board_pos].add_stats(atk, hp);
        state.board[board_pos].winners_bread_stacks += 1;
    }
}
