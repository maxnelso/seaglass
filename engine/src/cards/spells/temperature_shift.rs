//! `Temperature Shift` — Tier 4 Tavern spell (`4` Gold).
//!
//! Get a `Fire Baller` and a `Snow Baller`.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Temperature Shift`: get a `Fire Baller` and a `Snow Baller`.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    let mut fire = crate::cards::minions::fire_baller::template().instantiate();
    state.apply_global_unit_auras(&mut fire);
    state.add_to_hand(fire);
    let mut snow = crate::cards::minions::snow_baller::template().instantiate();
    state.apply_global_unit_auras(&mut snow);
    state.add_to_hand(snow);
}
