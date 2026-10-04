//! `Easterly Winds` — Tier 4 Tavern spell (`1` Gold).
//!
//! After each Refresh this game, give a random minion in the Tavern +9/+9.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Easterly Winds`: after each Refresh this game, give a random minion in the Tavern +9/+9.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(9, 9);
    state.auras.refresh_random_buffs.push((atk, hp));
}
