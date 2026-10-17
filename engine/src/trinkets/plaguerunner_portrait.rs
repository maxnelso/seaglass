//! `Plaguerunner Portrait` (`BG36_MagicItem_204`) — Greater, 2 Gold, Undead.
//!
//! Get a Plaguerunner. After you destroy a Plaguerunner outside combat, get a plain copy.
//!
//! Picked: a plain `Plaguerunner`. `trinket_tavern_destroy`: each `Plaguerunner` (Golden too)
//! destroyed in the Tavern, once its Deathrattle resolved, gives a plain one from the catalog, to
//! the hand or waiting for room.

use crate::cards::minions::plaguerunner;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133391;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_tavern_destroy(tavern_destroy),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, plaguerunner::ID);
}

fn tavern_destroy(state: &mut TavernState, destroyed: &Unit, _: &mut CardPool, _: &mut Rng) {
    if destroyed.card_id == plaguerunner::ID {
        super::give_minion(state, plaguerunner::ID);
    }
}
