//! `Defender's Rites` — Tier 4 Tavern spell (`2` Gold).
//!
//! Give a minion +7/+7 and Taunt.

use super::targeted_minion;
use crate::cards::CardHooks;
use crate::model::{Keyword, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_minion(cast)
}

/// `Defender's Rites`: give a minion +7/+7 and Taunt.
pub fn cast(state: &mut TavernState, _: &Unit, target: MinionSlot, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(7, 7);
    if let Some(target) = state.minion_mut(target) {
        target.add_stats(atk, hp);
        target.apply_keyword(Keyword::Taunt, false);
    }
}
