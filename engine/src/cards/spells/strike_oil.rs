//! `Strike Oil` — Tier 2 Tavern spell (`3` Gold).
//!
//! Gain 1 maximum Gold.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Strike Oil`: gain 1 maximum Gold.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.auras.base_max_gold_bonus += 1;
    state.max_gold += 1;
}
