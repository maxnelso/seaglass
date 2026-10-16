//! `Sellemental Portrait` (`BG32_MagicItem_831`) — Lesser, 4 Gold, Elemental.
//!
//! Get a Sellemental. At the start of each turn, get another.
//!
//! Picked, and at each turn start: a plain `Sellemental` from the catalog goes to the hand, or
//! waits for room in a full hand (`docs/trinkets.md` §5).

use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 120800;

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
    super::give_minion(state, cards::minions::sellemental::ID);
}
