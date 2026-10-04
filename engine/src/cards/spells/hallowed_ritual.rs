//! `Hallowed Ritual` — Tier 7 Tavern spell (`5` Gold).
//!
//! Discover a Tier 7 minion.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Hallowed Ritual`: Discover a Tier 7 minion.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    let mut opts = pool.draw_discover_options(7, 3, rng);
    for opt in &mut opts {
        state.apply_global_unit_auras(opt);
    }
    if !opts.is_empty() {
        state.push_discover(opts);
    }
}
