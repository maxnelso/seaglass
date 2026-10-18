//! `Open Bar` (`BGS_Treasures_023`) — Tier 4 Darkmoon Prize.
//!
//! Gain 5 free Refreshes. At the start of each turn, gain 5 more.
//!
//! 5 free Refreshes now, and a game-long player effect (one stack per cast): 5 more per stack at
//! each turn start (`player_turn_start`). Free Refreshes carry over.

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, EffectDuration, PlayerEffect, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65277;

/// The free Refreshes it gives, now and at each turn start.
const REFRESHES: u32 = 5;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast).on_player_turn_start(turn_start))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.auras.free_refreshes += REFRESHES;
    state.auras.add_effect(ID, 1, EffectDuration::Game);
}

fn turn_start(state: &mut TavernState, effect: &mut PlayerEffect) {
    state.auras.free_refreshes += REFRESHES * effect.stacks;
}
