//! `Boom's Monster Portrait` (`BG32_MagicItem_172`) — Greater, 6 Gold, Mech.
//!
//! Get a Dr. Boom's Monster. At the start of every turn, get another.
//!
//! Picked, and at each turn start: a plain `Dr. Boom's Monster`, a minion in no pool
//! (`docs/trinkets.md` §11.9), to the hand or waiting for room.

use crate::cards::minions::dr_booms_monster;
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 120866;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(give)
            .on_trinket_turn_start(give),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, dr_booms_monster::ID);
}
