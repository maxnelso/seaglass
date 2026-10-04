//! `Air Baller` (`BG36_181`) — Tier 4 Elemental (`6/6`).
//!
//! When you sell this, give your minions `+2/+2` (`+4/+4` if Golden) plus `baller_bonus`.
//! Improve your future Ballers (`baller_bonus += 1`, or `+= 2` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 401;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Air Baller", 6, 6, 4)
        .with_tribe(Tribe::Elemental)
        .on_sell(|state, sold, _, _| on_sell(state, sold))
}

pub fn on_sell(state: &mut TavernState, sold: &Unit) {
    let mult = if sold.is_golden { 2 } else { 1 };
    let amount = (2 + state.auras.baller_bonus) * mult;
    for b in &mut state.board {
        b.add_stats(amount, amount);
    }
    state.auras.baller_bonus += mult;
}
