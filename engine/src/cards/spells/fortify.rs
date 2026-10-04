//! `Fortify` — Tier 1 Tavern spell (`1` Gold).
//!
//! Give a minion +3 Health and Taunt.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::{Keyword, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Fortify`: give a minion +3 Health and Taunt.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, _: &mut Rng) {
    if board_pos < state.board.len() {
        let (atk, hp) = state.auras.spell_stat_buff(0, 3);
        state.board[board_pos].add_stats(atk, hp);
        state.board[board_pos].apply_keyword(Keyword::Taunt, false);
    }
}
