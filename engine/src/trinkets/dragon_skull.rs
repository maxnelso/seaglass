//! `Dragon Skull` (`BG36_MagicItem_203`) — Lesser, 2 Gold, Dragon.
//!
//! Whenever you trigger a Battlecry, give your left- and right-most minions +5/+5.
//!
//! `trinket_battlecry_triggered`: after each trigger of a friendly Battlecry in the Tavern (each
//! repeat too), the left-most and the right-most minions on the board gain +5/+5 (once with a
//! single minion; a minion being played isn't on the board yet).

use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 133390;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_battlecry_triggered(triggered),
    )]
}

fn triggered(state: &mut TavernState, _: &Unit) {
    let all: Vec<usize> = (0..state.board.len()).collect();
    for pos in super::ends(&all) {
        state.board[pos].add_stats(5, 5);
    }
}
