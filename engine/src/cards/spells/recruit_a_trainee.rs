//! `Recruit a Trainee` — Tier 1 Tavern spell (`2` Gold).
//!
//! Get a random Tier 1 minion.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Recruit a Trainee`: get a random Tier 1 minion.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    if state.hand.len() < 10 {
        if let Some(mut drawn) = pool.draw_from_pool(1, rng) {
            state.apply_global_unit_auras(&mut drawn);
            state.add_to_hand(drawn);
        }
    }
}
