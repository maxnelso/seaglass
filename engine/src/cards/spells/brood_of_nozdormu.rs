//! `Brood of Nozdormu` — Tier 5 Tavern spell (`2` Gold).
//!
//! At the start of your next combat, double your left-most minion's Attack.

use super::{record_effect, spell, SPELL_BROOD_OF_NOZDORMU};
use crate::cards::{CardHooks, CombatSides};
use crate::model::{EffectDuration, PlayerEffect, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast).on_player_start_of_combat(start_of_combat)
}

/// `Brood of Nozdormu`: at the start of your next combat, double your left-most minion's Attack.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    record_effect(state, SPELL_BROOD_OF_NOZDORMU, 1, EffectDuration::Turn);
}

/// `Brood of Nozdormu`: at the start of the next combat, double the Attack of your left-most
/// minion (once per stack).
fn start_of_combat(sides: &mut CombatSides<'_>, effect: &mut PlayerEffect) {
    for _ in 0..std::mem::take(&mut effect.stacks) {
        if let Some(add) = sides.board.first().map(|u| u.attack) {
            sides.buff_unit(0, add, 0, "Brood of Nozdormu");
        }
    }
}
