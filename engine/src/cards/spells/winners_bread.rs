//! `Winner's Bread` — Tier 2 Tavern spell (`2` Gold).
//!
//! Give a minion +2/+3; if you win your next combat, it gets a Blood Gem next turn.

use super::targeted_minion;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_minion(cast)
}

/// `Winner's Bread`: give a minion +2/+3; if you win your next combat, it gets a Blood Gem next
/// turn.
pub fn cast(state: &mut TavernState, _: &Unit, target: MinionSlot, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(2, 3);
    if let Some(target) = state.minion_mut(target) {
        target.add_stats(atk, hp);
        target.winners_bread_stacks += 1;
    }
}
