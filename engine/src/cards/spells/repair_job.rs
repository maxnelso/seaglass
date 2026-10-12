//! `Repair Job` — Tier 3 Tavern spell (`2` Gold).
//!
//! Give a minion +4/+8.

use super::targeted_minion;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_minion(cast)
}

/// `Repair Job`: give a minion +4/+8.
pub fn cast(state: &mut TavernState, _: &Unit, target: MinionSlot, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(4, 8);
    if let Some(target) = state.minion_mut(target) {
        target.add_stats(atk, hp);
    }
}
