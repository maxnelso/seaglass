//! `Southsea Busker` (`BG26_135`) — Tier 1 Pirate (`3/1`).
//! **Battlecry:** Gain `1` (`2` if Golden) Gold next turn.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 115;
pub const NAME: &str = "Southsea Busker";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 1, 1)
        .with_tribe(Tribe::Pirate)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let bonus = if unit.is_golden { 2 } else { 1 };
    state.bonus_gold_next_turn += bonus;
}
