//! `Search Through Time` — Tier 2 Tavern spell (`2` Gold).
//!
//! Discover a minion of your Tier; it is locked until next turn.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Search Through Time`: Discover a minion of your Tier; it is locked until next turn.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    let mut opts = pool.draw_discover_options(state.tavern_tier, 3, rng);
    for opt in &mut opts {
        state.apply_global_unit_auras(opt);
        opt.locked_turns = 1;
    }
    if !opts.is_empty() {
        state.push_discover(opts);
    }
}
