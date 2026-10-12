//! `Sacred Gift` — Tier 7 Tavern spell (`4` Gold).
//!
//! Give a minion Divine Shield.

use super::targeted_minion;
use crate::cards::CardHooks;
use crate::model::{Keyword, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_minion(cast)
}

/// `Sacred Gift`: give a minion Divine Shield.
pub fn cast(state: &mut TavernState, _: &Unit, target: MinionSlot, _: &mut CardPool, _: &mut Rng) {
    if let Some(target) = state.minion_mut(target) {
        target.apply_keyword(Keyword::DivineShield, false);
    }
}
