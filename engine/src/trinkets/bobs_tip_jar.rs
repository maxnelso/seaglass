//! `Bob's Tip Jar` (`BG30_MagicItem_996`) — Greater, 0 Gold.
//!
//! Gain 4 Gold. Increase your maximum Gold by 4.
//!
//! Picked: the player gains 4 Gold, and their maximum Gold goes up by 4 for the rest of the game
//! (`max_gold` and `base_max_gold_bonus`, as `Strike Oil`: `tavern.md` §3.1).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112988;

/// The Gold and maximum Gold it gives.
const GOLD: u32 = 4;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.gold += GOLD;
    state.max_gold += GOLD;
    state.auras.base_max_gold_bonus += GOLD;
}
