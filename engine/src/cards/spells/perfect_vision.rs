//! `Perfect Vision` — Tier 6 Tavern spell (`2` Gold).
//!
//! Set a minion's stats to 20/20.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Perfect Vision`: set a minion's stats to 20/20.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, _: &mut Rng) {
    if board_pos < state.board.len() {
        let (atk, hp) = state.auras.spell_stat_buff(20, 20);
        let target = &mut state.board[board_pos];
        target.attack = atk;
        target.health = hp;
        target.sync_max_stats();
        crate::cards::check_stat_thresholds(target);
    }
}
