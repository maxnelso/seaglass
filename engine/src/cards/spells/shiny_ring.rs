//! `Shiny Ring` — Tier 3 Tavern spell (`2` Gold).
//!
//! Give your minions +1/+1.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Shiny Ring` (and `Sludge Corrosion`): give your minions +1/+1.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(1, 1);
    for u in &mut state.board {
        u.add_stats(atk, hp);
    }
}
