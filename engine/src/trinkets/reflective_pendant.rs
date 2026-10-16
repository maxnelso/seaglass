//! `Reflective Pendant` (`BG30_MagicItem_706`) — Lesser, 6 Gold.
//!
//! Get a plain copy of a random friendly minion. At the start of each turn, repeat this.
//!
//! Picked, and at each turn start: a plain copy (`cards::plain_instance`) of a random minion on the
//! board (one `rng.below(k)`) goes to the hand, or waits for room in a full hand
//! (`docs/trinkets.md` §5). Nothing with an empty board.

use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 117857;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(give)
            .on_trinket_turn_start(give),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    if state.board.is_empty() {
        return;
    }
    let pick = rng.below(state.board.len());
    if let Some(copy) = cards::plain_instance(&state.board[pick]) {
        state.add_to_hand_or_wait(copy);
    }
}
