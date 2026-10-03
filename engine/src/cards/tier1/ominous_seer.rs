//! `Ominous Seer` (`BG31_330`) — Tier 1 Demon (`2/1`).
//! **Battlecry:** The next Tavern spell you buy costs `(1)` (`(2)` if Golden) less.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 120;
pub const NAME: &str = "Ominous Seer";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 1, 1).with_tribe(Tribe::Demon)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let discount = if unit.is_golden { 2 } else { 1 };
    state.auras.next_spell_discount += discount;
}
