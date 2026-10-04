//! `Firelands Fugitive` (`BG35_882`) — Tier 5 Elemental (`5/7`).
//!
//! Battlecry: Get a (`2` if Golden) `Conflagration`(s).

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 521;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Firelands Fugitive", 5, 7, 5).with_tribe(Tribe::Elemental)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        state.add_to_hand(tokens::make_conflagration());
    }
}
