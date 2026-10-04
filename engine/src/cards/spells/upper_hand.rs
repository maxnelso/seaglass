//! `Upper Hand` — Tier 5 Tavern spell (`3` Gold).
//!
//! At the start of your next combat, set a random enemy minion's Health to 1.

use super::{record_effect, spell, SPELL_UPPER_HAND};
use crate::cards::{CardHooks, CombatSides};
use crate::model::{EffectDuration, PlayerEffect, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast).on_player_start_of_combat(start_of_combat)
}

/// `Upper Hand`: at the start of your next combat, set a random enemy minion's Health to 1.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    record_effect(state, SPELL_UPPER_HAND, 1, EffectDuration::Turn);
}

/// `Upper Hand`: at the start of the next combat, set a random enemy minion's Health to 1 (once
/// per stack).
fn start_of_combat(sides: &mut CombatSides<'_>, effect: &mut PlayerEffect) {
    for _ in 0..std::mem::take(&mut effect.stacks) {
        let candidates: Vec<usize> = sides
            .enemy_board
            .iter()
            .enumerate()
            .filter(|(_, u)| u.health > 0)
            .map(|(i, _)| i)
            .collect();
        if !candidates.is_empty() {
            let idx = if candidates.len() == 1 {
                candidates[0]
            } else {
                candidates[sides.rng.below(candidates.len())]
            };
            let target = &mut sides.enemy_board[idx];
            target.health = 1;
            target.max_health = target.max_health.max(1);
        }
    }
}
