//! `Chef's Choice` — Tier 2 Tavern spell (`2` Gold).
//!
//! Get a different random minion of a friendly minion's type.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Chef's Choice`: get a different random minion of a friendly minion's type.
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if board_pos < state.board.len() && state.hand.len() < 10 {
        let target_tribe = state.board[board_pos].tribe;
        let exclude_id = state.board[board_pos].card_id;
        if let Some(mut drawn) =
            pool.draw_by_tribe(target_tribe, Some(exclude_id), state.tavern_tier, rng)
        {
            state.apply_global_unit_auras(&mut drawn);
            state.add_to_hand(drawn);
        }
    }
}
