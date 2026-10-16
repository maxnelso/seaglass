//! `Worn Treasure Map` (`BG32_MagicItem_428`) — Lesser, 0 Gold.
//!
//! In two turns, gain 10 Gold. (Once per game.)
//!
//! `trinket_turn_start` counts the turns started since it was picked (`counter`); at the 2nd (the
//! start of turn 8 for one picked on turn 6) the player gains 10 Gold, once (`stacks`).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112075;

/// Turns to wait.
const TURNS: i32 = 2;
/// The Gold it gives.
const GOLD: u32 = 10;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_turn_start(turn_start))]
}

fn turn_start(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let Some(map) = super::state_mut(&mut state.auras, ID).filter(|t| t.stacks == 0) else {
        return;
    };
    map.counter += 1;
    if map.counter >= TURNS {
        map.stacks = 1;
        state.gold += GOLD;
    }
}
