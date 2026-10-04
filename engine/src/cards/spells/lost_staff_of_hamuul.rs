//! `Lost Staff of Hamuul` — Tier 6 Tavern spell (`2` Gold).
//!
//! Refresh the Tavern with minions of a friendly minion's type.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Lost Staff of Hamuul`: refresh the Tavern with minions of a friendly minion's type.
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if board_pos < state.board.len() {
        let target_tribe = state.board[board_pos].tribe;
        state.refresh_shop_with_tribe(target_tribe, pool, rng);
    }
}
