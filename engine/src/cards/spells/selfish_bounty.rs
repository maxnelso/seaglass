//! `Selfish Bounty` — Tier 3 Tavern spell (`2` Gold).
//!
//! Give your left-most minion +6/+6.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Selfish Bounty`: give your left-most minion +6/+6.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    if !state.board.is_empty() {
        let (atk, hp) = state.auras.spell_stat_buff(6, 6);
        state.board[0].add_stats(atk, hp);
    }
}
