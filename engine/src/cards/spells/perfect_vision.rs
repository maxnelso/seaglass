//! `Perfect Vision` — Tier 6 Tavern spell (`2` Gold).
//!
//! Set a minion's stats to 20/20. Spell power adds to them (`21/22` with `Humon'gozz`). Set net of
//! the auras on it (`aura_applied` clears), which then apply again on top (a Volumizer with a
//! +0/+3 bonus becomes 20/23; in the Tavern, once bought): engine decision.

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
        target.until_next_turn_atk = 0;
        target.sync_max_stats();
        // Set net of its auras: they apply again on top (as `Robust Evolution`).
        target.aura_applied = (0, 0);
        crate::cards::check_stat_thresholds(target);
    }
    state.sync_all_auras();
}
