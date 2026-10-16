//! `Rascal Sticker` (`BG36_MagicItem_424`) — Lesser, 0 Gold, Pirate.
//!
//! Gain 1 Gold for each friendly Golden minion. At the start of your turn, repeat this.
//!
//! Picked, and at each turn start: 1 Gold for each Golden minion on the board (not the hand:
//! **engine decision**).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134640;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(gain)
            .on_trinket_turn_start(gain),
    )]
}

fn gain(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.gold += state.board.iter().filter(|u| u.is_golden).count() as u32;
}
