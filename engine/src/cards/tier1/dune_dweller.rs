//! `Dune Dweller` (`BG31_815`) — Tier 1 Elemental (`3/3`).
//! **Battlecry:** Give Elementals in the Tavern `+1/+1` (`+2/+2` if Golden) this game.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 109;
pub const NAME: &str = "Dune Dweller";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 1).with_tribe(Tribe::Elemental)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let mult = if unit.is_golden { 2 } else { 1 };
    state.auras.tavern_elemental_atk += mult;
    state.auras.tavern_elemental_hp += mult;

    // Immediately buff any Elementals currently in Bob's shop.
    for shop_unit in &mut state.shop {
        if shop_unit.tribe.matches(Tribe::Elemental) {
            shop_unit.add_stats(mult, mult);
        }
    }
}
