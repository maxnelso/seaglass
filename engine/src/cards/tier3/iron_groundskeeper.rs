//! `Iron Groundskeeper` (`BG27_000`) — Tier 3 Neutral (`2/2`).
//! **Battlecry:** Get `2` (`4` if Golden) copies of `Fortify`.

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 321;
pub const NAME: &str = "Iron Groundskeeper";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 2, 3).with_tribe(Tribe::None)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let count = if unit.is_golden { 4 } else { 2 };
    for _ in 0..count {
        state.add_to_hand(spells::make_fortify());
    }
}
