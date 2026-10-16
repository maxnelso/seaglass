//! `Glass of Perspective (Greater)` (`BG36_MagicItem_303t`) — Greater, 3 Gold, Quilboar.
//!
//! Get 2 random Choose One cards. At the start of each turn, get 2 more.
//!
//! As the Lesser one (`glass_of_perspective.rs`), 2 each time (one draw each).

use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133348;

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
    for _ in 0..2 {
        super::give_random_minion(state, rng, |t| cards::is_choose_one_minion(t.card_id));
    }
}
