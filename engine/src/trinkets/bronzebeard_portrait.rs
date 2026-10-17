//! `Bronzebeard Portrait` (`BG30_MagicItem_418`) — Greater, 3 Gold, Dragon, Murloc.
//!
//! Get a Brann Bronzebeard and a random Battlecry minion. Your Brann Bronzebeards are both Murlocs and Dragons.
//!
//! Picked: a plain `Brann Bronzebeard`, then a random Battlecry minion of the player's Tier or lower
//! from the catalog (`trinkets::give_random_minion`). `trinket_modifies`: every Brann Bronzebeard
//! is a Dragon and a Murloc (`Tribe::DragonMurloc`, `docs/trinkets.md` §11.8; the text is in its
//! file).

use crate::cards::minions::brann_bronzebeard;
use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 111607;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_modifies(|card| card == brann_bronzebeard::ID),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    super::give_minion(state, brann_bronzebeard::ID);
    super::give_random_minion(state, rng, |t| cards::is_battlecry_minion(t.card_id));
}
