//! `Azsharan Cutlassier` (`BG33_830`) — Tier 3 Pirate (`6/4`).
//! **Battlecry:** Your Tavern spells give an extra `+1` (`+2` if Golden) Attack this game.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 306;
pub const NAME: &str = "Azsharan Cutlassier";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 6, 4, 3)
        .with_tribe(Tribe::Pirate)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let delta = if unit.is_golden { 2 } else { 1 };
    state.auras.spell_bonus_atk += delta;
}
