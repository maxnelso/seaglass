//! `Snow Baller` (`BG31_818`) — Tier 2 Elemental (`3/4`).
//! When you sell this, give your minions `+1` (`+2` if Golden) Health. Improve your future Ballers.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 227;
pub const NAME: &str = "Snow Baller";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 4, 2)
        .with_tribe(Tribe::Elemental)
        .on_sell(|state, sold, _, _| on_sell(state, sold))
}

pub fn on_sell(state: &mut TavernState, sold: &Unit) {
    let mult = if sold.is_golden { 2 } else { 1 };
    let hp_buff = (1 + super::fire_baller::baller_bonus(&state.auras)) * mult;
    for u in &mut state.board {
        u.add_stats(0, hp_buff);
    }
    super::fire_baller::improve_ballers(&mut state.auras, mult);
}
