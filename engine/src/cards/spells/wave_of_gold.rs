//! `Wave of Gold` — Tier 5 Tavern spell (`2` Gold).
//!
//! Give your minions +3/+2 (twice for Golden minions).

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Wave of Gold`: give your minions +3/+2 (twice for Golden minions).
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(3, 2);
    for u in &mut state.board {
        u.add_stats(atk, hp);
        if u.is_golden {
            u.add_stats(atk, hp);
        }
    }
}
