//! `Nerglish Phrasebook (Greater)` (`BG30_MagicItem_914t`) — Greater, 1 Gold, Murloc.
//!
//! After you play a minion, give the left-most minion in your hand +6/+6.
//!
//! As the Lesser one (`nerglish_phrasebook.rs`), +6/+6.

use crate::cards::{CardHooks, Played};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 115237;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_play(after_play))]
}

fn after_play(state: &mut TavernState, played: &Played, _: &mut CardPool, _: &mut Rng) {
    if played.from_hand {
        super::buff_left_most_in_hand(state, 6, 6);
    }
}
