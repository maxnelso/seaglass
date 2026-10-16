//! `Orc-estra Portrait` (`BGDUO_MagicItem_007`) — Lesser, 5 Gold, Duos only.
//!
//! Get an Orc-estra Conductor. At the start of each turn, get another.
//!
//! Picked, and at each turn start: a plain `Orc-estra Conductor` from the catalog goes to the
//! hand, or waits for room in a full hand.

use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 117938;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(give)
            .on_trinket_turn_start(give),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, cards::minions::orc_estra_conductor::ID);
}
