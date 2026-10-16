//! `Volumizer Portrait (Greater)` (`BG36_MagicItem_423t`) — Greater, 3 Gold, Mech.
//!
//! Get 2 random Magnetic Volumizers. At the start of each turn, repeat this.
//!
//! Picked, and at each turn start: 2 random Volumizer(s) (`Blue`, `Green` or `Red Volumizer`,
//! each one `rng.below(3)`), plain from the catalog, go to the hand, or wait for room in a full hand
//! (`docs/trinkets.md` §5).

use crate::cards::minions::{blue_volumizer, green_volumizer, red_volumizer};
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134461;

/// Volumizers each time.
const COUNT: usize = 2;

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
