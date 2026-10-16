//! `Beatboxer Portrait` (`BG35_MagicItem_741`) — Greater, 6 Gold, Mech.
//!
//! Get a Polarizing Beatboxer.
//!
//! Picked: a plain `Polarizing Beatboxer` from the catalog goes to the hand, or waits for room in
//! a full hand (`docs/trinkets.md` §5).

use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 131139;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, cards::minions::polarizing_beatboxer::ID);
}
