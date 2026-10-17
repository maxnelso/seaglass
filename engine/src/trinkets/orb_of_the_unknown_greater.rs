//! `Orb of the Unknown (Greater)` (`BG35_MagicItem_816t`) — Greater, 0 Gold.
//!
//! Get a random Greater Trinket and 4 Gold.
//!
//! As the Lesser one: 4 Gold, then it is replaced by a random Greater trinket the player doesn't
//! own (`docs/trinkets.md` §3.5) **(engine decision)**.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131328;

/// The Gold it gives.
const GOLD: u32 = 4;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    state.gold += GOLD;
    super::replace_with_random(state, ID, true, Tribe::None, pool, rng);
}
