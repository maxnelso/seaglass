//! `Fandral's Fortune` — Tier 6 Tavern spell (`3` Gold).
//!
//! Discover a Choose One card with both effects combined.

use super::{draw_discover_choose_one, spell};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Fandral's Fortune`: Discover a Choose One card with both effects combined.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    let opts = draw_discover_choose_one(state, 3, rng);
    if !opts.is_empty() {
        state.push_discover(opts);
    }
}
