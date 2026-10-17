//! `Mindslasher Portrait` (`BG36_MagicItem_402`) — Lesser, 4 Gold.
//!
//! Get a Vicious Mindslasher. Your Vicious Mindslashers also give stats to adjacent minions.
//!
//! Picked: a plain `Vicious Mindslasher`. `trinket_modifies`: the stats a Vicious Mindslasher gains
//! from a Tavern spell also go to its adjacent minions (in combat, for that combat)
//! (`docs/trinkets.md` §11.8; the text is in its file).

use crate::cards::minions::vicious_mindslasher;
use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134638;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_modifies(|card| card == vicious_mindslasher::ID),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, vicious_mindslasher::ID);
}
