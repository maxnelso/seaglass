//! `Bluegill Flippers` (`BG32_MagicItem_893`) — Lesser, 2 Gold, Murloc.
//!
//! Whenever you cast a Tavern spell, give the left-most minion in your hand and warband +3/+3.
//!
//! `trinket_spell_cast`: each Tavern spell cast gives the left-most minion in the hand (spells
//! skipped) and the left-most minion on the board +3/+3 each.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 121140;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_spell_cast(spell_cast))]
}

fn spell_cast(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::buff_left_most_in_hand(state, 3, 3);
    if let Some(unit) = state.board.first_mut() {
        unit.add_stats(3, 3);
    }
}
