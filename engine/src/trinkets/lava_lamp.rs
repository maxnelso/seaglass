//! `Lava Lamp` (`BG30_MagicItem_951`) — Greater, 2 Gold, Elemental.
//!
//! After you sell 6 minions, get a random Elemental. (6 left!)
//!
//! `trinket_after_sell`: every 6th minion sold (`counter`, across turns) gives a random Elemental
//! of the player's Tier or lower from the game's catalog (`trinkets::give_random_minion`), which
//! waits for room in a full hand.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 111499;

/// Minions to sell per Elemental.
const SALES: i32 = 6;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_sell(after_sell))]
}

fn after_sell(state: &mut TavernState, _: &Unit, _: &mut CardPool, rng: &mut Rng) {
    if super::count_to(&mut state.auras, ID, SALES) {
        super::give_random_minion(state, rng, |t| t.tribe.matches(Tribe::Elemental));
    }
}
