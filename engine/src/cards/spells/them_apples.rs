//! `Them Apples` — Tier 1 Tavern spell (`1` Gold).
//!
//! Give the minions in the Tavern +1/+2.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Them Apples`: give the minions in the Tavern +1/+2.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(1, 2);
    for u in &mut state.shop {
        if !u.is_spell {
            u.add_stats(atk, hp);
        }
    }
}
