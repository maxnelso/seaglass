//! `Weighted Gauntlet` (`BG36_MagicItem_414`) — Greater, 3 Gold.
//!
//! At the end of your turn, give your minions +7/+7. (Doubles at the start of each turn!)
//!
//! `trinket_end_turn`: every minion on the board gains `+N/+N`, `N = 7 * 2^counter`; each turn
//! start (`trinket_turn_start`) doubles it (`counter` + 1), from the turn after the pick, without
//! a cap (confirmed): `N` saturates at `i32::MAX`, and stats saturate there (`docs/tavern.md`
//! §4.2).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134444;

/// The initial buff before any turn-start doubling.
const BASE: i32 = 7;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(double)
            .on_trinket_end_turn(|s, n, p, r| super::repeat_end_turn(s, n, p, r, end_turn)),
    )]
}

fn double(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.counter = t.counter.saturating_add(1);
    }
}

fn end_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let Some(t) = super::state(&state.auras, ID) else {
        return;
    };
    let bonus = if t.counter >= 31 {
        i32::MAX
    } else {
        BASE.saturating_mul(1i32 << t.counter.max(0))
    };
    for unit in &mut state.board {
        unit.add_stats(bonus, bonus);
    }
}
