//! `Manipulator Portrait` (`BG30_MagicItem_876`) — Greater, 1 Gold.
//!
//! Get a Faceless Manipulator.
//!
//! Picked: a plain `Faceless Manipulator` (`docs/trinkets.md` §11.9: a minion in no pool, its text in its file).

use crate::cards::minions::faceless_manipulator;
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 111207;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, faceless_manipulator::ID);
}
