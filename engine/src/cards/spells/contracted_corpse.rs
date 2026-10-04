//! `Contracted Corpse` — Tier 5 Tavern spell (`3` Gold).
//!
//! Discover a Deathrattle minion.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Contracted Corpse`: Discover a Deathrattle minion.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    let mut opts = pool.draw_discover_filtered(
        state.tavern_tier,
        3,
        crate::cards::is_deathrattle_minion,
        rng,
    );
    for opt in &mut opts {
        state.apply_global_unit_auras(opt);
    }
    if !opts.is_empty() {
        state.push_discover(opts);
    }
}
