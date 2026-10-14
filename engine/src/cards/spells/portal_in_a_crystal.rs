//! `Portal in a Crystal` (`BG31_244`) — Tier 5 Tavern spell (`2` Gold), Duos only.
//!
//! Pass a friendly non-Golden minion.
//!
//! The minion leaves the board, without going back to the pool, and is Passed with its stats, a
//! free Pass that counts as the team's (`docs/duos_tavern.md` §8.3). Only Duos shops offer it.

use super::targeted;
use crate::cards::{self, CardHooks};
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast).on_spell_target_filter(|unit| !unit.is_golden)
}

/// `Portal in a Crystal`: Pass the friendly minion.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, _: &mut Rng) {
    cards::pass_board_minion(state, board_pos);
}
