//! `Hostile Bounty` — Tier 3 Tavern spell (`2` Gold).
//!
//! Give 4 random friendly minions +4 Attack.

use super::{buff_random_friendly, spell};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Hostile Bounty`: give 4 random friendly minions +4 Attack.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    buff_random_friendly(state, 4, 4, 0, rng);
}
