//! `Sacred Gift` — Tier 7 Tavern spell (`4` Gold).
//!
//! Give a minion Divine Shield.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::{Keyword, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Sacred Gift`: give a minion Divine Shield.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, _: &mut Rng) {
    if board_pos < state.board.len() {
        state.board[board_pos].apply_keyword(Keyword::DivineShield, false);
    }
}
