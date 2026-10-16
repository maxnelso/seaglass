//! `Goblin Wallet` (`BG30_MagicItem_847`) — Lesser, 1 Gold.
//!
//! At the end of each turn, increase your maximum Gold by 1.
//!
//! `trinket_end_turn`: `max_gold` and `base_max_gold_bonus` go up by 1, as `Strike Oil`'s (above
//! 10 too: **engine decision**), so the next turns refill 1 more Gold.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 111448;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_end_turn(end_turn))]
}

fn end_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.max_gold += 1;
    state.auras.base_max_gold_bonus += 1;
}
