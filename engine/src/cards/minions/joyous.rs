//! `Joyous` (`BG36_110`) — Tier 1 Aberration (`2/3`).
//! **Battlecry:** Give your **Deity** `+2/+1` (`+4/+2` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 101;
pub const NAME: &str = "Joyous";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 3, 1)
        .with_tribe(Tribe::Aberration)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let mult = if unit.is_golden { 2 } else { 1 };
    state.auras.deity.attack += 2 * mult;
    state.auras.deity.health += mult;
}
