//! `Miniature Ship` (`BG35_MagicItem_710`) — Greater, 1 Gold, Pirate.
//!
//! After you cast a Tavern spell, give your Pirates +3/+3.
//!
//! `trinket_spell_cast`: each Tavern spell cast gives every Pirate on the board +3/+3.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130892;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_spell_cast(spell_cast))]
}

fn spell_cast(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    for unit in &mut state.board {
        if unit.tribe.matches(Tribe::Pirate) {
            unit.add_stats(3, 3);
        }
    }
}
