//! `Armor Stash` — Tier 5 Tavern spell (`3` Gold).
//!
//! Set your Armor to 5.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Armor Stash`: set your Armor to 5.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.armor = 5;
}
