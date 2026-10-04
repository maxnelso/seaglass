//! `Seafood Stew` — Tier 3 Tavern spell (`2` Gold).
//!
//! Give a minion +1/+1, plus +1/+1 per bonus keyword among your minions.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::{Unit, BONUS_KEYWORDS};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Seafood Stew`: give a minion +1/+1, plus +1/+1 per bonus keyword among your minions.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, _: &mut Rng) {
    if board_pos < state.board.len() {
        let kw_count = BONUS_KEYWORDS
            .iter()
            .filter(|&&kw| state.board.iter().any(|u| u.has_keyword(kw)))
            .count();
        let repeats = (1 + kw_count) as i32;
        let (atk, hp) = state.auras.spell_stat_buff(1, 1);
        state.board[board_pos].add_stats(atk * repeats, hp * repeats);
    }
}
