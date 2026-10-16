//! `Glowing Crystal` (`BG36_MagicItem_220`) — Lesser, 0 Gold.
//!
//! At the start of your turn, gain 1 Gold for each different friendly minion type.
//!
//! `trinket_turn_start`: 1 Gold per type the board covers by the Menagerie rule (confirmed: a
//! maximum matching of minions to types, each minion counting for one type at most,
//! `types::menagerie_type_count`, as `Menagerie Tableware`'s).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};
use crate::types;

pub const ID: CardId = 133696;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_turn_start(turn_start))]
}

fn turn_start(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.gold += types::menagerie_type_count(&state.board) as u32;
}
