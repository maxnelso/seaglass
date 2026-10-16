//! `Replica Cathedral` (`BG30_MagicItem_434`) — Lesser, 3 Gold.
//!
//! Your first spell each turn casts an extra time.
//!
//! `trinket_extra_spell_casts`: the first spell the player casts in the Tavern each turn (from the
//! hand or by an effect: **engine decision**; `armed`, which starts again each turn) is cast once
//! more, an extra cast as `Brann`-style repeats are.

use crate::cards::CardHooks;
use crate::model::{CardId, TrinketState};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112855;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_extra_spell_casts(extra_casts),
    )]
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn extra_casts(trinket: &mut TrinketState) -> u32 {
    if trinket.armed != 0 {
        return 0;
    }
    trinket.armed = 1;
    1
}
