//! `Careful Investment` — Tier 3 Tavern spell (`1` Gold).
//!
//! Get 2 extra Gold next turn.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Careful Investment`: get 2 extra Gold next turn.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.bonus_gold_next_turn += 2;
}
