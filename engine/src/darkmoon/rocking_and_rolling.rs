//! `Rocking and Rolling` (`BGS_Treasures_029`) — Tier 1 Darkmoon Prize.
//!
//! For the rest of the game, gain 1 free Refresh at the start of each turn.
//!
//! A game-long player effect (one stack per cast): from the next turn start on, each turn start
//! gives 1 free Refresh per stack (`player_turn_start`), which carries over as every free Refresh
//! does (engine decision).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, EffectDuration, PlayerEffect, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 66427;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast).on_player_turn_start(turn_start))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.auras.add_effect(ID, 1, EffectDuration::Game);
}

fn turn_start(state: &mut TavernState, effect: &mut PlayerEffect) {
    state.auras.free_refreshes += effect.stacks;
}
