//! `Glass of Perspective` (`BG36_MagicItem_303`) — Lesser, 2 Gold, Quilboar.
//!
//! Get a random Choose One card. At the start of each turn, get another.
//!
//! Picked, and at each turn start: a random Choose One minion of the player's Tier or lower from
//! the game's catalog (`trinkets::give_random_minion`, one draw) goes to the hand, or waits for
//! room in a full hand.

use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133346;

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
    super::give_random_minion(state, rng, |t| cards::is_choose_one_minion(t.card_id));
}
