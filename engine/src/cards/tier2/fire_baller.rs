//! `Fire Baller` (`BG31_816`) — Tier 2 Elemental (`4/3`).
//! When you sell this, give your minions `+1` (`+2` if Golden) Attack. Improve your future Ballers.

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 211;
pub const NAME: &str = "Fire Baller";
/// Card counter shared by all Ballers (`Fire`, `Snow` and `Air Baller`): how much future
/// Ballers are improved.
pub const BALLER_COUNTER: CardId = ID;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 3, 2)
        .with_tribe(Tribe::Elemental)
        .on_sell(|state, sold, _, _| on_sell(state, sold))
}

pub fn on_sell(state: &mut TavernState, sold: &Unit) {
    let mult = if sold.is_golden { 2 } else { 1 };
    let atk_buff = (1 + baller_bonus(&state.auras)) * mult;
    for u in &mut state.board {
        u.add_stats(atk_buff, 0);
    }
    improve_ballers(&mut state.auras, mult);
}

/// How much future Ballers are improved ([`BALLER_COUNTER`]).
pub fn baller_bonus(auras: &PlayerAuras) -> i32 {
    auras.counter(BALLER_COUNTER)
}

/// Improve future Ballers by `n`.
pub fn improve_ballers(auras: &mut PlayerAuras, n: i32) {
    auras.add_counter(BALLER_COUNTER, n);
}
