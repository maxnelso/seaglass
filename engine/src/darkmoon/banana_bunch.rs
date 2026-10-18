//! `Banana Bunch` (`BGS_Treasures_040`) — Tier 1 Darkmoon Prize.
//!
//! Get 2 Tavern Dish Bananas.
//!
//! Two `Tavern Dish Banana`s (the Tier 1 Tavern spell; `King Mukla`'s Bananas are other cards),
//! each to the hand or waiting for room (`docs/darkmoon_prizes.md` §5).

use crate::cards::spells::{self, spell};
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 92337;

/// Bananas it gives.
const COUNT: usize = 2;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    for _ in 0..COUNT {
        state.add_to_hand_or_wait(spells::make_tavern_dish_banana());
    }
}
