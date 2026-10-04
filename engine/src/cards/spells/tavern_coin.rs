//! `Tavern Coin` — Tier 1 Tavern spell (`1` Gold).
//!
//! Gain 1 Gold.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Tavern Coin` (and `Hasty Excavation`): gain 1 Gold.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.gold += 1;
}
