//! `Fire Baller` (`BG31_816`) — Tier 2 Elemental (`4/3`).
//! When you sell this, give your minions `+1` (`+2` if Golden) Attack. Improve your future Ballers.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 211;
pub const NAME: &str = "Fire Baller";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 3, 2).with_tribe(Tribe::Elemental)
}

pub fn on_sell(state: &mut TavernState, sold: &Unit) {
    let mult = if sold.is_golden { 2 } else { 1 };
    let atk_buff = (1 + state.auras.baller_bonus) * mult;
    for u in &mut state.board {
        u.add_stats(atk_buff, 0);
    }
    state.auras.baller_bonus += mult;
}
