//! `Azeroth Model Globe` (`BG30_MagicItem_425`) — Lesser, 0 Gold.
//!
//! At the start of every two turns, gain 2 Gold and Discover a Tier 6 minion. (2 turns left!)
//!
//! Every 2nd turn start after the pick (`counter`: turn 8, 10, ... for one picked on turn 6):
//! 2 Gold, then a Discover of 3 Tier 6 minions from the pool (`CardPool::draw_discover_options`,
//! as a Triple Reward: the highest lower Tier with copies left if none).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112001;

/// Turns between rewards.
const TURNS: i32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_turn_start(turn_start))]
}

fn turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if super::count_to(&mut state.auras, ID, TURNS) {
        state.gold += 2;
        let options = pool.draw_discover_options(6, 3, rng);
        state.push_discover(options);
    }
}
