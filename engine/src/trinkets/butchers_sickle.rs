//! `Butcher's Sickle` (`BG30_MagicItem_406`) — Greater, 1 Gold, Undead.
//!
//! Get a Butchering. At the start of each turn, get another.
//!
//! Picked, and at each turn start: a `Butchering` (a fresh copy of the Tavern spell) goes to the
//! hand, or waits for room in a full hand (`docs/trinkets.md` §5).

use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 111502;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(give)
            .on_trinket_turn_start(give),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if let Some(spell) = cards::spells::spell_by_id(cards::spells::SPELL_BUTCHERING) {
        state.add_to_hand_or_wait(spell);
    }
}
