//! `Orb of the Unknown` (`BG35_MagicItem_816`) — Lesser, 0 Gold.
//!
//! Get a random Lesser Trinket.
//!
//! Picked, it is replaced by a random Lesser trinket the player doesn't own
//! (`trinkets::replace_with_random`, `docs/trinkets.md` §3.5), whose own text resolves: the Orb
//! becomes it rather than a third trinket joining **(engine decision)**.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130834;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    super::replace_with_random(state, ID, false, Tribe::None, pool, rng);
}
