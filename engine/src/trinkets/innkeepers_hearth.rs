//! `Innkeeper's Hearth` (`BG32_MagicItem_362t`) — Greater, 0 Gold.
//!
//! Discover two Tier 6 minions. Set their stats to 30/30.
//!
//! Picked: two Discovers, one after the other, of 3 Tier 6 minions each from the pool
//! (`CardPool::draw_discover_options`, as a Triple Reward), whose options are `30/30` (set before
//! the choice: **engine decision**).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 121684;

/// The stats the options are set to.
const STATS: i32 = 30;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    for _ in 0..2 {
        let mut options = pool.draw_discover_options(6, 3, rng);
        for option in &mut options {
            option.attack = STATS;
            option.health = STATS;
            option.sync_max_stats();
        }
        state.push_discover(options);
    }
}
