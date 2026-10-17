//! `Pagle's Fishing Rod` (`BG30_MagicItem_993`) — Greater, 3 Gold.
//!
//! Get a random Tier 7 minion. At the start of each turn, get another.
//!
//! Picked, and at each turn start: a random minion of exactly Tier 7 from the game's catalog (one
//! `rng.below(k)`), to the hand, or waiting for room in a full hand (`docs/trinkets.md` §5;
//! confirmed: Tier 7 is in the game).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 113103;

/// The Tier of the minion it gives.
const TIER: u32 = 7;

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
    super::give_random_minion_of_tier(state, rng, TIER);
}
