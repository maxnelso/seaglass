//! `Baller Portrait (Greater)` (`BG35_MagicItem_861`) — Greater, 5 Gold, Elemental.
//!
//! Get a Temperature Shift. After you play 11 Elementals, repeat this. (11 left!)
//!
//! Picked: a `Temperature Shift` goes to the hand, or waits for room in a full hand.
//! `trinket_after_play`: every 11th Elemental played from the hand (`counter`, across turns), another.

use crate::cards::{spells, CardHooks, Played};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130873;

/// Elementals to play per Temperature Shift.
const ELEMENTALS: i32 = 11;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(give)
            .on_trinket_after_play(after_play),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if let Some(spell) = spells::spell_by_id(spells::SPELL_TEMPERATURE_SHIFT) {
        state.add_to_hand_or_wait(spell);
    }
}

fn after_play(state: &mut TavernState, played: &Played, pool: &mut CardPool, rng: &mut Rng) {
    if !played.from_hand || played.magnetized || !played.tribe.matches(Tribe::Elemental) {
        return;
    }
    if super::count_to(&mut state.auras, ID, ELEMENTALS) {
        give(state, pool, rng);
    }
}
