//! `Lovesick Balladist` (`BG26_814`) — Tier 4 Pirate (`3/4`).
//!
//! Battlecry: Give a Pirate `+2` Health (`twice` if Golden). (Improved by each Gold you spent this turn!)

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 439;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Lovesick Balladist", 3, 4, 4)
        .with_tribe(Tribe::Pirate)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let repeats = if unit.is_golden { 2 } else { 1 };
    let hp_gain = (2 + state.gold_spent_this_turn as i32) * repeats;
    if let Some(pirate) = state
        .board
        .iter_mut()
        .find(|u| u.tribe.matches(Tribe::Pirate))
    {
        pirate.add_stats(0, hp_gain);
    }
}
