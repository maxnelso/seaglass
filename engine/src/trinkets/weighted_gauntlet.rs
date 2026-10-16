//! `Weighted Gauntlet` (`BG36_MagicItem_414`) — Lesser, 3 Gold.
//!
//! At the end of your turn, give your minions +1/+1. (Doubles at the start of each turn!)
//!
//! `trinket_end_turn`: every minion on the board gains `+N/+N`, `N = 2^counter`; each turn start
//! (`trinket_turn_start`) doubles it (`counter` + 1), from the turn after the pick. It stops
//! doubling at `+1024/+1024` (10 doublings), to keep stats in range (**engine decision**).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134444;

/// Doublings after which the bonus stays.
const MAX_DOUBLINGS: i32 = 10;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(double)
            .on_trinket_end_turn(end_turn),
    )]
}

fn double(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.counter = (t.counter + 1).min(MAX_DOUBLINGS);
    }
}

fn end_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let Some(t) = super::state(&state.auras, ID) else {
        return;
    };
    let bonus = 1 << t.counter.clamp(0, MAX_DOUBLINGS);
    for unit in &mut state.board {
        unit.add_stats(bonus, bonus);
    }
}
