//! `Corrupted Coin` — Tier 5 Tavern spell (`2` Gold).
//!
//! Gain 2 Gold.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast).on_discarded(discarded)
}

/// `Corrupted Coin`: gain 2 Gold.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.gold += 2;
}

/// `Corrupted Coin` discarded: gain 2 maximum Gold.
fn discarded(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    state.auras.base_max_gold_bonus += 2;
    state.max_gold += 2;
}
