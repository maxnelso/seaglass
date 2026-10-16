//! `Enigmatic Headstone` (`BG32_MagicItem_276`) — Lesser, 0 Gold, Undead.
//!
//! At the end of each turn, your Undead have +2 Attack this game (wherever they are).
//!
//! `trinket_end_turn`: the Undead Attack bonus (`auras.undead_bonus_attack`, as `Nerubian
//! Deathswarmer`'s) goes up by 2, and the auras resync.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 121121;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_end_turn(end_turn))]
}

fn end_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::buff_undead_attack(state, 2);
}
