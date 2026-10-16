//! `Flighty Portrait` (`BG36_MagicItem_820`) — Lesser, 2 Gold, Murloc.
//!
//! Get a Flighty Scout. After you cast a Tavern spell, give Flighty Scouts in your hand and warband +3/+3.
//!
//! Picked: a plain `Flighty Scout` goes to the hand, or waits for room in a full hand.
//! `trinket_spell_cast`: each Tavern spell cast gives every `Flighty Scout` (Golden too) in the
//! hand and on the board +3/+3.

use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133064;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_spell_cast(spell_cast),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, cards::minions::flighty_scout::ID);
}

fn spell_cast(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let scout = cards::minions::flighty_scout::ID;
    for unit in state.hand.iter_mut().chain(&mut state.board) {
        if !unit.is_spell && unit.card_id == scout {
            unit.add_stats(3, 3);
        }
    }
}
