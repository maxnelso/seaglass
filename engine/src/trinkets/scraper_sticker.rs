//! `Scraper Sticker` (`BG35_MagicItem_301`) — Lesser, 1 Gold, Mech.
//!
//! Get a random Magnetic Mech. At the start of each turn, get another.
//!
//! Picked, and at each turn start: a random Magnetic Mech of the player's Tier or lower from the
//! game's catalog (**engine decision**), one `rng.below(k)`, goes to the hand, or waits for room
//! in a full hand (`docs/trinkets.md` §5).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130879;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(give)
            .on_trinket_turn_start(give),
    )]
}

fn give(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    super::give_random_minion(state, rng, |t| {
        t.tribe.matches(crate::model::Tribe::Mech) && t.instantiate().magnetic
    });
}
