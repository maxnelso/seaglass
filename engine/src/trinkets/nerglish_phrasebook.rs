//! `Nerglish Phrasebook` (`BG30_MagicItem_914`) — Lesser, 0 Gold, Murloc.
//!
//! After you play a minion, give the left-most minion in your hand +3/+3.
//!
//! `trinket_after_play`: after each minion played from the hand (a Magnetization too), the
//! left-most minion in the hand (spells skipped) gains +3/+3. Nothing without one.

use crate::cards::{CardHooks, Played};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 110991;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_play(after_play))]
}

fn after_play(state: &mut TavernState, played: &Played, _: &mut CardPool, _: &mut Rng) {
    if played.from_hand {
        super::buff_left_most_in_hand(state, 3, 3);
    }
}
