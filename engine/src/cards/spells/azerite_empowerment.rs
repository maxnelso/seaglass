//! `Azerite Empowerment` — Tier 6 Tavern spell (`4` Gold).
//!
//! Give your minions +2/+2 twice.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Azerite Empowerment`: give your minions +2/+2 twice.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(2, 2);
    for _ in 0..2 {
        for u in &mut state.board {
            u.add_stats(atk, hp);
        }
    }
}
