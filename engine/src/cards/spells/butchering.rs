//! `Butchering` — Tier 5 Tavern spell (`3` Gold).
//!
//! Destroy a friendly Undead; your Undead have +8 Attack for the rest of the game.

use super::targeted_tribe;
use crate::cards::CardHooks;
use crate::model::{Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_tribe(Tribe::Undead, cast)
}

/// `Butchering`: destroy a friendly Undead; your Undead have +8 Attack for the rest of the game.
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if board_pos < state.board.len() && state.board[board_pos].tribe.matches(Tribe::Undead) {
        state.destroy_board_unit(board_pos, pool, rng);
        let (atk, _hp) = state.auras.spell_stat_buff(8, 0);
        state.auras.undead_bonus_attack += atk;
        state.sync_all_auras();
    }
}
