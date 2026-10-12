//! `Perfect Vision` — Tier 6 Tavern spell (`2` Gold).
//!
//! Set a minion's stats to 20/20.

use super::targeted_minion;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_minion(cast)
}

/// `Perfect Vision`: set a minion's stats to 20/20.
pub fn cast(state: &mut TavernState, _: &Unit, target: MinionSlot, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(20, 20);
    if let Some(target) = state.minion_mut(target) {
        target.attack = atk;
        target.health = hp;
        target.sync_max_stats();
        crate::cards::check_stat_thresholds(target);
    }
}
