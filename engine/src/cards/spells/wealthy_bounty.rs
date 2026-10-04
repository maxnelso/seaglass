//! `Wealthy Bounty` — Tier 3 Tavern spell (`2` Gold).
//!
//! Gain 2 Gold.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Wealthy Bounty`: gain 2 Gold.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.gold += 2;
}
