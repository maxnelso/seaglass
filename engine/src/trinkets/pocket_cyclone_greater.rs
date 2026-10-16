//! `Pocket Cyclone (Greater)` (`BG35_MagicItem_850t`) — Greater, 0 Gold, Elemental.
//!
//! Cast Easterly Winds four times. At the start of each turn, cast it twice more.
//!
//! As the Lesser one (`pocket_cyclone.rs`): 4 casts as picked, 2 at each turn start.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131329;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_turn_start(turn_start),
    )]
}

fn bought(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    super::cast_easterly_winds(state, 4, pool, rng);
}

fn turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    super::cast_easterly_winds(state, 2, pool, rng);
}
