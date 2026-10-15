//! `Rock Master Voone` (`BG26_HERO_104`) — Armor 15 (Duos 13).
//!
//! Hero Power: `Upbeat Harmony` (`BG26_HERO_104p`), 0 Gold: At the end of every 3 turns, get a plain copy of the left- most card in your hand. (3 turns left!)
//!
//! Passive. Every 3rd turn end (`hero_end_turn`, `counter`, counting from the first) gives the
//! player a plain copy of the left-most card in their hand (`cards::plain_instance`; a spell: a
//! fresh one) if the hand has room; with an empty hand, nothing (engine decision).

use crate::cards::{self, spells, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 99033;
pub const POWER: CardId = 99034;

/// Turn ends per copy.
const TURNS: i32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_end_turn(end_turn))]
}

/// End of turn: every 3rd, a plain copy of the left-most card in the hand.
fn end_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter < TURNS {
        return;
    }
    power.counter = 0;
    let Some(first) = state.hand.first() else {
        return;
    };
    let copy = if first.is_spell {
        spells::spell_by_id(first.card_id)
    } else {
        cards::plain_instance(first)
    };
    if let Some(copy) = copy.filter(|_| super::hand_has_room(state)) {
        state.add_to_hand(copy);
    }
}
