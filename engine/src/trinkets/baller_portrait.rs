//! `Baller Portrait` (`BG36_MagicItem_390`) — Lesser, 2 Gold, Elemental.
//!
//! Get a Fire Baller or a Snow Baller. At the start of each turn, repeat this.
//!
//! Picked, and at each turn start: a plain `Fire Baller` or `Snow Baller` (one `rng.below(2)`, at
//! random: **engine decision**) goes to the hand, or waits for room in a full hand.

use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134199;

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
    let ballers = [
        cards::minions::fire_baller::ID,
        cards::minions::snow_baller::ID,
    ];
    super::give_minion(state, ballers[rng.below(2)]);
}
