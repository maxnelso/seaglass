//! `Friendly Bounty` — Tier 3 Tavern spell (`2` Gold).
//!
//! Get a random minion of your most common type.
//!
//! Naga counts (`docs/tavern.md` §7.8): with a `Firescale Hoarder` and an `Ominous Seer`, it gets
//! a Naga (or an All minion).

use super::{most_common_tribe_in_pool, spell};
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
        let tribe = most_common_tribe_in_pool(&state.board, Some(pool), rng);
        if let Some(mut drawn) = pool.draw_by_tribe(tribe, None, state.tavern_tier, rng) {
            state.apply_global_unit_auras(&mut drawn);
            state.add_to_hand(drawn);
        }
    }
}
