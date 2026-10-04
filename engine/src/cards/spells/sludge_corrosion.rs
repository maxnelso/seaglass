//! `Sludge Corrosion` (`BG36_301t`) — Tier 4 token spell (`1` Gold).
//!
//! Give your minions +1/+1. If you discard this, cast it twice.

use super::{cast_spell, shiny_ring, spell};
use crate::cards::tokens::make_sludge_corrosion;
use crate::cards::{CardFlags, CardHooks};
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(shiny_ring::cast)
        .with_flags(CardFlags::NOT_IN_POOL)
        .on_discarded(discarded)
}

/// Discarded: cast it twice.
fn discarded(state: &mut TavernState, _: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    for _ in 0..2 {
        state.auras.spells_played += 1;
        cast_spell(state, make_sludge_corrosion(), 0, pool, rng);
    }
}
