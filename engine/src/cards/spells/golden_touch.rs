//! `Golden Touch` (`BG28_830`) — Tier 5 token spell (`5` Gold).
//!
//! Make a random minion in the Tavern Golden.

use super::spell;
use crate::cards::{CardFlags, CardHooks};
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast).with_flags(CardFlags::NOT_IN_POOL)
}

/// `Golden Touch`: make a random minion in the Tavern Golden.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    let candidates: Vec<usize> = state
        .shop
        .iter()
        .enumerate()
        .filter(|(_, u)| !u.is_spell && !u.is_golden)
        .map(|(i, _)| i)
        .collect();
    if !candidates.is_empty() {
        let pick = if candidates.len() == 1 {
            candidates[0]
        } else {
            candidates[rng.below(candidates.len())]
        };
        state.shop[pick].make_golden();
        crate::cards::sync_unit_auras(&mut state.shop[pick], &state.auras);
    }
}
