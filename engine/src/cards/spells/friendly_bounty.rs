//! `Friendly Bounty` — Tier 3 Tavern spell (`2` Gold).
//!
//! Get a random minion of your most common type.

use super::{most_common_tribe, spell};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Friendly Bounty`: get a random minion of your most common type.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    if state.hand.len() < 10 {
        let tribe = most_common_tribe(&state.board, rng);
        if let Some(mut drawn) = pool.draw_by_tribe(tribe, None, state.tavern_tier, rng) {
            state.apply_global_unit_auras(&mut drawn);
            state.add_to_hand(drawn);
        }
    }
}
