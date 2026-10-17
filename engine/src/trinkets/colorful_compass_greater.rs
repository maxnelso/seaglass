//! `Colorful Compass (Greater)` (`BG30_MagicItem_426t`) — Greater, 2 Gold.
//!
//! Get 2 random 92. At the start of each turn, get 2 more.
//!
//! As the Lesser one (`docs/trinkets.md` §3.4, confirmed): 2 random minions of the type it names,
//! picked and at each turn start.

use crate::cards::{CardFlags, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 115250;

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
    super::give_random_of_named_type(state, ID, 2, rng);
}
