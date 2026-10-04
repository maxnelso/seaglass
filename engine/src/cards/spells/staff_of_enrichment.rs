//! `Staff of Enrichment` — Tier 3 Tavern spell (`2` Gold).
//!
//! Minions in the Tavern have +2/+2 for the rest of the game.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Staff of Enrichment`: minions in the Tavern have +2/+2 for the rest of the game.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(2, 2);
    state.auras.tavern_all_atk += atk;
    state.auras.tavern_all_hp += hp;
    for u in &mut state.shop {
        if !u.is_spell {
            u.add_stats(atk, hp);
        }
    }
}
