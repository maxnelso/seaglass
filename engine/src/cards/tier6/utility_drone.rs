//! `Utility Drone` (`BG26_152`) — Tier 6 Mech (`4/5`).
//!
//! At the end of your turn, give your minions `+4/+5` (`+8/+10` if Golden) for each Magnetization they have.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::{CardPool, TavernState};
use crate::rng::Rng;

pub const ID: CardId = 629;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Utility Drone", 4, 5, 6)
        .with_tribe(Tribe::Mech)
        .on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let mult = if state.board[self_idx].is_golden { 2 } else { 1 };
    for u in &mut state.board {
        if u.magnetizations_count > 0 {
            let count = u.magnetizations_count as i32;
            u.add_stats(4 * mult * count, 5 * mult * count);
        }
    }
}
