//! `Friends and Family Discount` (`BGS_Treasures_022`) — Tier 4 Darkmoon Prize.
//!
//! Minions in the Tavern cost (2) this game.
//!
//! A game-long player effect: a minion's price is 2 Gold (`player_minion_cost`), whatever the
//! Hero Power's price; `next_minion_discount` and the trinkets still apply after it.

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, EffectDuration, PlayerEffect, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65270;

/// A minion's price.
const PRICE: u32 = 2;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast).on_player_minion_cost(minion_cost))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    state.auras.add_effect(ID, 1, EffectDuration::Game);
}

fn minion_cost(_: &PlayerEffect, _: u32) -> u32 {
    PRICE
}
