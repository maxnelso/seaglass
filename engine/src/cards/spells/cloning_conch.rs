//! `Cloning Conch` — Tier 4 Tavern spell (`4` Gold).
//!
//! Get a random Murloc and a copy of it.

use super::spell;
use crate::cards::CardHooks;
use crate::model::{Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Cloning Conch`: get a random Murloc and a copy of it.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(mut drawn) = pool.draw_by_tribe(Tribe::Murloc, None, state.tavern_tier, rng) {
        state.apply_global_unit_auras(&mut drawn);
        let copy = drawn.clone();
        state.add_to_hand(drawn);
        state.add_to_hand(copy);
    }
}
