//! `Misplaced Tea Set` — Tier 4 Tavern spell (`3` Gold).
//!
//! Give a friendly minion of each type +4/+4.

use super::{apply_misplaced_tea_set, spell};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Misplaced Tea Set`: give a friendly minion of each type +4/+4.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    apply_misplaced_tea_set(state, rng);
}
