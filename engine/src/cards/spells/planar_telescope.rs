//! `Planar Telescope` — Tier 3 Tavern spell (`4` Gold).
//!
//! Discover a minion of your most common type.

use super::{most_common_tribe, spell};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Planar Telescope`: Discover a minion of your most common type.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    let tribe = most_common_tribe(&state.board, rng);
    let mut opts = pool.draw_discover_by_tribe(tribe, state.tavern_tier, 3, rng);
    for opt in &mut opts {
        state.apply_global_unit_auras(opt);
    }
    if !opts.is_empty() {
        state.push_discover(opts);
    }
}
