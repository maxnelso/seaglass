//! `Tomb Turning` — Tier 4 Tavern spell (`2` Gold).
//!
//! Discover an Undead; it dies if played this turn.

use super::spell;
use crate::cards::CardHooks;
use crate::model::{Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Tomb Turning`: Discover an Undead; it dies if played this turn.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    let mut opts = pool.draw_discover_by_tribe(Tribe::Undead, state.tavern_tier, 3, rng);
    for opt in &mut opts {
        state.apply_global_unit_auras(opt);
        opt.dies_on_play_this_turn = true;
    }
    if !opts.is_empty() {
        state.push_discover(opts);
    }
}
