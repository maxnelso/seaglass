//! `Energizing Chamber` — Tier 5 Tavern spell (`1` Gold).
//!
//! Give your Deity +7/+7.

use super::{cast_spell, spell, spell_by_id, SPELL_ENERGIZING_CHAMBER};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast).on_discarded(discarded)
}

/// `Energizing Chamber`: give your Deity +7/+7.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    let (atk, hp) = state.auras.spell_stat_buff(7, 7);
    state.auras.deity.attack += atk;
    state.auras.deity.health += hp;
}

/// `Energizing Chamber` discarded: cast it twice.
fn discarded(state: &mut TavernState, _: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(chamber) = spell_by_id(SPELL_ENERGIZING_CHAMBER) {
        for _ in 0..2 {
            state.auras.spells_played += 1;
            cast_spell(state, chamber.clone(), 0, pool, rng);
        }
    }
}
