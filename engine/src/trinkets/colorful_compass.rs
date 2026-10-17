//! `Colorful Compass` (`BG30_MagicItem_426`) — Lesser, 2 Gold.
//!
//! Get a random 92. At the start of each turn, get another.
//!
//! The `92` is the minion type the trinket names (`TrinketState::tribe`), assigned when it is
//! offered: the player's most common type then (`docs/trinkets.md` §3.4, confirmed). Picked, and
//! at each turn start: a random minion of that type, of the player's Tier or lower, from the
//! game's catalog (`trinkets::give_random_minion`), to the hand or waiting for room.

use crate::cards::{CardFlags, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112002;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::TRINKET_MINION_TYPE)
            .on_trinket_bought(give)
            .on_trinket_turn_start(give),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    super::give_random_of_named_type(state, ID, 1, rng);
}
