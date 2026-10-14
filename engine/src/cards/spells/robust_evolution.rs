//! `Robust Evolution` — Tier 3 Tavern spell (`1` Gold).
//!
//! Transform a minion into a random minion of the next Tier, keeping its stats. The new minion is
//! never Golden (a Golden target keeps its doubled stats), and a Tier 6 target becomes a random
//! Tier 6 minion. It keeps the stats without the auras on them: it gets the auras it qualifies for
//! itself (an Undead gets the Undead Attack once, a non-Undead loses it).

use super::targeted_minion;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_minion(cast)
}

/// `Robust Evolution`: transform a minion into a random minion of the next Tier, keeping its stats.
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    target: MinionSlot,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if let Some(old) = state.minion(target).cloned() {
        let target_tier = (old.tavern_tier + 1).min(6);
        let mut opts = pool.draw_discover_options(target_tier, 1, rng);
        if let Some(mut evolved) = opts.pop() {
            pool.return_unit(&old);
            // The stats without the auras on them: the sync below gives the new minion its own.
            let (aura_atk, aura_hp) = old.aura_applied;
            evolved.attack = old.attack - aura_atk;
            evolved.health = (old.health - aura_hp).max(1);
            evolved.max_attack = evolved.attack.max(evolved.base_attack);
            evolved.max_health = evolved.health.max(evolved.base_health);
            crate::cards::check_stat_thresholds(&mut evolved);
            if let Some(target) = state.minion_mut(target) {
                *target = evolved;
            }
            state.sync_all_auras();
        }
    }
}
