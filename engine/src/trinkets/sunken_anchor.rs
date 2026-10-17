//! `Sunken Anchor` (`BG35_MagicItem_890`) — Lesser, 6 Gold, Pirate.
//!
//! Get 2 random Bounties. At the start of each turn, repeat this.
//!
//! Picked, and at each turn start: 2 Bounty Tavern spells, each one of the 5 at random
//! (`spells::draw_random_bounty`, as `Bigwig Bandit`'s), to the hand or waiting for room.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130890;

/// The Bounties it gives each time.
const COUNT: usize = 2;

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
    give_bounties(state, COUNT, rng);
}

/// `count` random Bounty spells to the hand, or waiting for room (the Bounty trinkets).
pub fn give_bounties(state: &mut TavernState, count: usize, rng: &mut Rng) {
    for _ in 0..count {
        state.add_to_hand_or_wait(crate::cards::spells::draw_random_bounty(rng));
    }
}
