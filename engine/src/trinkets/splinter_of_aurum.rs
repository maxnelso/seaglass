//! `Splinter of Aurum` (`BG32_MagicItem_350`) — Lesser, 2 Gold, Pirate.
//!
//! When you have 15 Gold, get a random Golden Tier 5 minion. (Once per game.)
//!
//! As picked, at each turn start and after each action (`trinket_after_action`): with 15 Gold or
//! more, a Golden copy of a random Tier 5 minion of the game's catalog (one draw; a new copy, as
//! `Lockbox`'s Golden minion) goes to the hand, or waits for room in a full hand, once per game
//! (`armed`; only once a minion was given: confirmed).

use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 111387;

/// The Gold it waits for.
const GOLD: u32 = 15;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(check)
            .on_trinket_turn_start(check)
            .on_trinket_after_action(check),
    )]
}

fn check(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    if state.gold < GOLD || super::state(&state.auras, ID).is_none_or(|t| t.armed != 0) {
        return;
    }
    let tier5: Vec<_> = cards::game_templates(&state.auras)
        .iter()
        .filter(|t| t.tavern_tier == 5 && !t.intrinsic_golden)
        .collect();
    if tier5.is_empty() {
        return;
    }
    let mut golden = tier5[rng.below(tier5.len())].instantiate();
    golden.make_golden();
    golden.intrinsic_golden = false;
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.armed = 1;
    }
    state.add_to_hand_or_wait(golden);
}
