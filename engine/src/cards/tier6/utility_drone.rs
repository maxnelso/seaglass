//! `Utility Drone` (`BG26_152`) — Tier 6 Mech (`4/5`).
//!
//! At the end of your turn, give your minions `+4/+5` (`+8/+10` if Golden) for each Magnetization they have.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 629;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Utility Drone", 4, 5, 6).with_tribe(Tribe::Mech)
}

pub fn on_end_turn(state: &mut TavernState) {
    let mut mult = 0i32;
    for u in &state.board {
        if u.card_id == ID {
            mult += if u.is_golden { 2 } else { 1 };
        }
    }
    if mult == 0 {
        return;
    }
    for u in &mut state.board {
        if u.magnetizations_count > 0 {
            let count = u.magnetizations_count as i32;
            u.add_stats(4 * mult * count, 5 * mult * count);
        }
    }
}
