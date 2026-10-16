//! `Bloodfury Shield` (`BG36_MagicItem_830`) — Greater, 2 Gold, Demon.
//!
//! Whenever you cast a Tavern spell, add a Fodder to your next Refresh.
//!
//! `trinket_spell_cast`: each cast of a Tavern spell (each repeat too) adds a Demon Fodder to the
//! next Refresh (`tokens::add_fodder_to_refreshes`, as Twisted Wrathguard's: the turn start's new
//! shop is one).

use crate::cards::{tokens, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133066;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_spell_cast(cast))]
}

fn cast(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    tokens::add_fodder_to_refreshes(&mut state.auras, 1, 1);
}
