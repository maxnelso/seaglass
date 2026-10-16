//! `Booty Bay Brew (Greater)` (`BG30_MagicItem_924t`) — Greater, 1 Gold, Pirate.
//!
//! Whenever you spend Gold, give two friendly Pirates +6/+6.
//!
//! As the Lesser one (`booty_bay_brew.rs`), +6/+6.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 115245;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_gold_spent(gold_spent))]
}

fn gold_spent(state: &mut TavernState, _: u32, _: &mut CardPool, rng: &mut Rng) {
    super::buff_random_pirates(state, 2, 6, 6, rng);
}
