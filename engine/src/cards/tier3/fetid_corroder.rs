//! `Fetid Corroder` (`BG36_112`) — Tier 3 Aberration (`3/3`).
//! **Battlecry:** Get a (`2` if Golden) `Sludge Corrosion`(s).

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 315;
pub const NAME: &str = "Fetid Corroder";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 3).with_tribe(Tribe::Aberration)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        if state.hand.len() < 10 {
            state.hand.push(tokens::make_sludge_corrosion());
        }
    }
}
