//! `Shath'Yar Shrine` (`BG36_MagicItem_406`) — Greater, 4 Gold.
//!
//! After you discard a spell, get a random Aberration.
//!
//! `trinket_after_discard`: each spell discarded from the hand gives a random Aberration of the
//! player's Tier or lower from the game's catalog (`trinkets::give_random_minion`), which waits
//! for room in a full hand.

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134319;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_discard(after_discard))]
}

fn after_discard(state: &mut TavernState, card: &Unit, _: &mut CardPool, rng: &mut Rng) {
    if card.is_spell {
        super::give_random_minion(state, rng, |t| t.tribe.matches(Tribe::Aberration));
    }
}
