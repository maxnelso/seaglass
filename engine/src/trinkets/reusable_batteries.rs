//! `Reusable Batteries` (`BG32_MagicItem_278`) — Lesser, 0 Gold, Mech.
//!
//! The first time you buy a minion each turn, get a Magnetic Satellite with its stats.
//!
//! `trinket_after_buy`: the first minion bought each turn (`armed`, which starts again each turn)
//! gives a `Satellite` (the 4/4 Magnetic Mech token) with the bought minion's Attack and Health as
//! its own (base) stats, which waits for room in a full hand.

use crate::cards::{tokens, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 121125;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_after_buy(after_buy),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    if bought.is_spell {
        return;
    }
    let Some(t) = super::state_mut(&mut state.auras, ID).filter(|t| t.armed == 0) else {
        return;
    };
    t.armed = 1;
    let mut satellite = tokens::make_satellite(false);
    // Its own stats (base ones too, so a Triple of Satellites doubles them).
    satellite.attack = bought.attack;
    satellite.health = bought.health.max(1);
    satellite.base_attack = satellite.attack;
    satellite.base_health = satellite.health;
    satellite.sync_max_stats();
    state.add_to_hand_or_wait(satellite);
}
