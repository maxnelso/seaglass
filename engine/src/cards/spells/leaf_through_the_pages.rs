//! `Leaf Through the Pages` — Tier 2 Tavern spell (`1` Gold).
//!
//! Get 2 free Refreshes.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Leaf Through the Pages`: get 2 free Refreshes.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.auras.free_refreshes += 2;
}
