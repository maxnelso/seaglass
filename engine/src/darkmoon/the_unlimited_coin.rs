//! `The Unlimited Coin` (`BGS_Treasures_014`) — Tier 2 Darkmoon Prize.
//!
//! Gain 1 Gold. Return this to your hand at end of turn.
//!
//! 1 Gold, and a turn-long player effect (one stack per cast): when the player ends the turn
//! (`player_end_turn`), a new `The Unlimited Coin` per stack goes to the hand, or waits for room
//! (`docs/darkmoon_prizes.md` §5).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, EffectDuration, PlayerEffect, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65256;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast).on_player_end_turn(end_turn))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.gold += 1;
    state.auras.add_effect(ID, 1, EffectDuration::Turn);
}

fn end_turn(state: &mut TavernState, effect: &mut PlayerEffect, _: &mut CardPool, _: &mut Rng) {
    for _ in 0..std::mem::take(&mut effect.stacks) {
        if let Some(coin) = super::make_prize(ID) {
            state.add_to_hand_or_wait(coin);
        }
    }
}
