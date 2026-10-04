//! `Blood Gem Barrage` — Tier 4 Tavern spell (`1` Gold).
//!
//! After each Refresh this game, play 2 Blood Gems on every minion in the Tavern.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Blood Gem Barrage`: after each Refresh this game, play 2 Blood Gems on every minion in the
/// Tavern.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.auras.refresh_blood_gems += 2;
}
