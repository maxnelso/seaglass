//! `Behemoth Portrait` (`BG32_MagicItem_998`) — Greater, 0 Gold, Elemental.
//!
//! Get an Arcane Behemoth.
//!
//! Picked: a plain `Arcane Behemoth` (`docs/trinkets.md` §11.9: a minion in no pool, its text in its file).

use crate::cards::minions::arcane_behemoth;
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 121114;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, arcane_behemoth::ID);
}
