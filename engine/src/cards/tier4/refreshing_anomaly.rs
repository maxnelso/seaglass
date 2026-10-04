//! `Refreshing Anomaly` (`BGS_116`) — Tier 4 Elemental (`4/5`).
//!
//! Battlecry: Gain 2 (`4` if Golden) free Refreshes.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 448;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Refreshing Anomaly", 4, 5, 4).with_tribe(Tribe::Elemental)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    state.auras.free_refreshes += if unit.is_golden { 4 } else { 2 };
}
