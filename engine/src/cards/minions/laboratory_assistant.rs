//! `Laboratory Assistant` (`BG35_150`) — Tier 2 Demon (`3/4`).
//! **Battlecry:** Add a (`two` if Golden) **Fodder(s)** to your next 3 **Refreshes**.

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 216;
pub const NAME: &str = "Laboratory Assistant";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 4, 2)
        .with_tribe(Tribe::Demon)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let count = if unit.is_golden { 2 } else { 1 };
    tokens::add_fodder_to_refreshes(&mut state.auras, count, 3);
}
