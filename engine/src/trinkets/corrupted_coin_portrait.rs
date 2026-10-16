//! `Corrupted Coin Portrait` (`BG36_MagicItem_400`) — Lesser, 3 Gold.
//!
//! Get 2 Corrupted Coins.
//!
//! Picked: 2 `Corrupted Coin`s (fresh copies of the Tavern spell) go to the hand, or wait for room
//! in a full hand (`docs/trinkets.md` §5).

use crate::cards::{self, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 134315;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(give))]
}

fn give(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    for _ in 0..2 {
        if let Some(spell) = cards::spells::spell_by_id(cards::spells::SPELL_CORRUPTED_COIN) {
            state.add_to_hand_or_wait(spell);
        }
    }
}
