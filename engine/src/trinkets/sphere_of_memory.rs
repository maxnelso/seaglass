//! `Sphere of Memory` (`BG36_MagicItem_372`) — Greater, 2 Gold.
//!
//! At the end of your turn, get 3 copies of the last Tavern spell you cast.
//!
//! `trinket_end_turn`: 3 fresh copies of the last Tavern spell the player cast
//! (`auras.last_tavern_spell_cast`) go to the hand, or wait for room in a full hand. Nothing
//! before any was cast.

use crate::cards::{spells, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133712;

/// Copies each turn.
const COPIES: usize = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_end_turn(|s, n, p, r| super::repeat_end_turn(s, n, p, r, end_turn)),
    )]
}

fn end_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let Some(spell) = state
        .auras
        .last_tavern_spell_cast
        .and_then(spells::spell_by_id)
    else {
        return;
    };
    for _ in 0..COPIES {
        state.add_to_hand_or_wait(spell.clone());
    }
}
