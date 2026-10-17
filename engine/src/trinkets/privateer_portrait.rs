//! `Privateer Portrait` (`BG35_MagicItem_712`) — Greater, 1 Gold, Pirate.
//!
//! Get a Proud Privateer and 2 random Bounties. At the start of each turn, get another 2 Bounties.
//!
//! Picked: a plain `Proud Privateer`, then 2 random Bounty Tavern spells (as `Sunken Anchor`'s);
//! at each turn start, 2 more.

use crate::cards::minions::proud_privateer;
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130891;

/// The Bounties it gives each time.
const COUNT: usize = 2;

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
    super::give_minion(state, proud_privateer::ID);
    turn_start(state, pool, rng);
}

fn turn_start(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    super::sunken_anchor::give_bounties(state, COUNT, rng);
}
