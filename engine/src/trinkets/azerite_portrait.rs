//! `Azerite Portrait` (`BG30_MagicItem_431`) — Greater, 5 Gold, Elemental.
//!
//! Get a Living Azerite. Your Living Azerites also give stats to friendly Elementals.
//!
//! Picked: a plain `Living Azerite` from the catalog. `trinket_modifies`: a Living Azerite's
//! Tavern spell cast also gives its stats to the friendly Elementals on the board (in combat, on
//! the combat board, for that combat) (`docs/trinkets.md` §11.8; the text is in its file).

use crate::cards::minions::living_azerite;
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112400;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_modifies(|card| card == living_azerite::ID),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, living_azerite::ID);
}
