//! `Chromatic Tear` (`BG35_MagicItem_840`) — Lesser, 3 Gold, Dragon.
//!
//! Get a random Chromadrake. At the start of each turn, repeat this.
//!
//! Picked, and at each turn start: one of the 5 `Chromadrake` tokens at random
//! (`tokens::CHROMADRAKE_IDS`, as `Hired Mount`'s), to the hand or waiting for room.

use crate::cards::{tokens, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130886;

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
    give_chromadrakes(state, 1, rng);
}

/// `count` random Chromadrakes (each one of the 5 at random) to the hand, or waiting for room.
pub fn give_chromadrakes(state: &mut TavernState, count: usize, rng: &mut Rng) {
    for _ in 0..count {
        let id = tokens::CHROMADRAKE_IDS[rng.below(tokens::CHROMADRAKE_IDS.len())];
        state.add_to_hand_or_wait(tokens::make_chromadrake(id, false));
    }
}
