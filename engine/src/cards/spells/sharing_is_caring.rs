//! `Sharing is Caring` — Tier 7 Tavern spell (`2` Gold).
//!
//! At the start of your next combat, give your left-most minion the stats of the nearest enemy
//! minion.

use super::{record_effect, spell, SPELL_SHARING_IS_CARING};
use crate::cards::{CardHooks, CombatSides};
use crate::model::{EffectDuration, PlayerEffect, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast).on_player_start_of_combat(start_of_combat)
}

/// `Sharing is Caring`: at the start of your next combat, give your left-most minion the stats of
/// the nearest enemy minion.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    record_effect(state, SPELL_SHARING_IS_CARING, 1, EffectDuration::Turn);
}

/// `Sharing is Caring`: at the start of the next combat, give your left-most minion the stats of
/// the nearest enemy minion (once per stack).
fn start_of_combat(sides: &mut CombatSides<'_>, effect: &mut PlayerEffect) {
    for _ in 0..std::mem::take(&mut effect.stacks) {
        if !sides.board.is_empty() {
            if let Some((add_atk, add_hp)) = sides
                .enemy_board
                .iter()
                .find(|u| u.health > 0)
                .map(|u| (u.attack.max(0), u.health.max(0)))
            {
                sides.buff_unit(0, add_atk, add_hp, "Sharing is Caring");
            }
        }
    }
}
