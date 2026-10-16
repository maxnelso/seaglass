//! `Sludge Portrait` (`BG36_MagicItem_430`) — Greater, 5 Gold.
//!
//! Get a Sludge Corrosion. After you discard a spell, get a Sludge Corrosion.
//!
//! Picked, and after each spell discarded from the hand (`trinket_after_discard`): a `Sludge
//! Corrosion` goes to the hand, or waits for room in a full hand.

use crate::cards::{spells, tokens, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134443;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(give)
            .on_trinket_after_discard(after_discard),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if let Some(spell) = spells::spell_by_id(tokens::SPELL_SLUDGE_CORROSION) {
        state.add_to_hand_or_wait(spell);
    }
}

fn after_discard(state: &mut TavernState, card: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    if card.is_spell {
        give(state, pool, rng);
    }
}
