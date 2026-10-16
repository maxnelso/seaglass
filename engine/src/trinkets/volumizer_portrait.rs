//! `Volumizer Portrait` (`BG36_MagicItem_423`) — Lesser, 3 Gold, Mech.
//!
//! Get a random Magnetic Volumizer. At the start of each turn, get another.
//!
//! Picked, and at each turn start: 1 random Volumizer(s) (`Blue`, `Green` or `Red Volumizer`,
//! each one `rng.below(3)`), plain from the catalog, go to the hand, or wait for room in a full hand
//! (`docs/trinkets.md` §5).

use crate::cards::minions::{blue_volumizer, green_volumizer, red_volumizer};
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134439;

/// Volumizers each time.
const COUNT: usize = 1;

/// The Magnetic Volumizers.
pub const VOLUMIZERS: [CardId; 3] = [blue_volumizer::ID, green_volumizer::ID, red_volumizer::ID];

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
    for _ in 0..COUNT {
        super::give_minion(state, VOLUMIZERS[rng.below(VOLUMIZERS.len())]);
    }
}
