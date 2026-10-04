//! `Overconfidence` — Tier 3 Tavern spell (`1` Gold).
//!
//! After your next combat, get 3 Gold next turn if you won (1 if tied).

use super::{record_effect, spell, SPELL_OVERCONFIDENCE};
use crate::cards::CardHooks;
use crate::model::{CombatResult, EffectDuration, PlayerEffect, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast).on_player_after_combat(after_combat)
}

/// `Overconfidence`: after your next combat, get 3 Gold next turn if you won (1 if tied).
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    record_effect(state, SPELL_OVERCONFIDENCE, 1, EffectDuration::Turn);
}

/// `Overconfidence`: after the next combat, get 3 Gold next turn per stack if you won (1 if
/// tied).
fn after_combat(state: &mut TavernState, effect: &mut PlayerEffect, result: CombatResult) {
    let stacks = std::mem::take(&mut effect.stacks);
    match result {
        CombatResult::Won => state.bonus_gold_next_turn += 3 * stacks,
        CombatResult::Tied => state.bonus_gold_next_turn += stacks,
        CombatResult::Lost => {}
    }
}
