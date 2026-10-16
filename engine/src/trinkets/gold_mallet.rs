//! `Gold Mallet` (`BG36_MagicItem_302`) — Lesser, 2 Gold, Pirate.
//!
//! At the end of your turn, give your minions +1/+1. (Improved by Golden minions you've played this game!)
//!
//! `trinket_after_play`: each Golden minion played from the hand (a Golden Magnetic minion
//! Magnetized from it too) adds one to `counter`. `trinket_end_turn`: every minion on the board
//! gains +1/+1 × (1 + `counter`): each Golden minion played adds the base amount again
//! (confirmed).

use crate::cards::{CardHooks, Played};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133342;

/// The base buff.
const BUFF: (i32, i32) = (1, 1);

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_after_play(after_play)
            .on_trinket_end_turn(|s, n, p, r| super::repeat_end_turn(s, n, p, r, end_turn)),
    )]
}

fn after_play(state: &mut TavernState, played: &Played, _: &mut CardPool, _: &mut Rng) {
    super::count_golden_play(state, ID, played);
}

fn end_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let times = 1 + super::state(&state.auras, ID).map_or(0, |t| t.counter);
    for unit in &mut state.board {
        unit.add_stats(BUFF.0.saturating_mul(times), BUFF.1.saturating_mul(times));
    }
}
