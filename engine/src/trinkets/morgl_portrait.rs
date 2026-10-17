//! `Morgl Portrait` (`BG32_MagicItem_926`) — Greater, 1 Gold, Murloc.
//!
//! Get a Tide Oracle Morgl.
//!
//! Picked: a plain `Tide Oracle Morgl` (`docs/trinkets.md` §11.9: a minion in no pool, its text in its file).

use crate::cards::minions::tide_oracle_morgl;
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 121111;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, tide_oracle_morgl::ID);
}
