//! `Czarina Portrait` (`BG32_MagicItem_283`) — Greater, 1 Gold, Mech.
//!
//! Get a Charging Czarina. Your Charging Czarinas also give Health.
//!
//! Picked: a plain `Charging Czarina`. `trinket_modifies`: a Charging Czarina gives as much Health
//! as Attack (`docs/trinkets.md` §11.8; the text is in its file).

use crate::cards::minions::charging_czarina;
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 121130;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_modifies(|card| card == charging_czarina::ID),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, charging_czarina::ID);
}
