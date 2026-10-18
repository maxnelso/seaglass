//! `Reserve Prices` (`BGS_Treasures_104`) — Tier 3 Darkmoon Prize.
//!
//! Tavern spells cost (1) less this turn.
//!
//! A turn-long player effect (one stack per cast): every spell in the Tavern costs 1 less per
//! stack (never below 0; a Health-cost spell 1 less Health) until the turn ends
//! (`player_spell_cost`).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, EffectDuration, PlayerEffect, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 129198;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast).on_player_spell_cost(spell_cost))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.auras.add_effect(ID, 1, EffectDuration::Turn);
}

fn spell_cost(effect: &PlayerEffect, cost: u32) -> u32 {
    cost.saturating_sub(effect.stacks)
}
