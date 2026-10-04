//! `En-Djinn Blazer` (`BG34_865`) — Tier 4 Elemental (`5/5`).
//!
//! Battlecry: After the Tavern is Refreshed this game, give a random minion in it `+10/+10` (`twice` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 417;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "En-Djinn Blazer", 5, 5, 4)
        .with_tribe(Tribe::Elemental)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        state.auras.refresh_random_buffs.push((10, 10));
    }
}
