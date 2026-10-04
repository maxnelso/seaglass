//! `Natural Blessing` — Tier 4 Tavern spell (`2` Gold).
//!
//! Give minions of a friendly minion's type +2/+1, on your board and in the Tavern.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::{Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Natural Blessing`: give minions of a friendly minion's type +2/+1, on your board and in the
/// Tavern.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, _: &mut Rng) {
    if board_pos < state.board.len() {
        let target_tribe = state.board[board_pos].tribe;
        if target_tribe != Tribe::None {
            let (atk, hp) = state.auras.spell_stat_buff(2, 1);
            for u in &mut state.board {
                if u.tribe.matches(target_tribe) {
                    u.add_stats(atk, hp);
                }
            }
            for u in &mut state.shop {
                if !u.is_spell && u.tribe.matches(target_tribe) {
                    u.add_stats(atk, hp);
                }
            }
        }
    }
}
