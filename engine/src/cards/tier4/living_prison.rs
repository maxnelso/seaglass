//! `Living Prison` (`BG36_180`) — Tier 4 Elemental (`4/5`).
//!
//! Activate (1): Gain (`double` if Golden) the stats of the next minion you buy this turn.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 438;
pub const ACTIVATE_COST: u32 = 1;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Living Prison", 4, 5, 4)
        .with_tribe(Tribe::Elemental)
        .with_activate_cost(ACTIVATE_COST)
        .on_activate(|state, source_pos, _, _, _| on_activate(state, source_pos))
        .on_minion_bought(on_minion_bought)
        .on_reset_turn_charges(|u| u.charges = 0)
}

pub fn on_activate(state: &mut TavernState, source_pos: usize) {
    if source_pos < state.board.len() {
        let mult = if state.board[source_pos].is_golden { 2 } else { 1 };
        state.board[source_pos].charges += mult;
    }
}

/// Gain the stats of the bought minion once per Activation.
pub fn on_minion_bought(state: &mut TavernState, self_idx: usize, bought: &mut Unit) {
    let prison = &mut state.board[self_idx];
    let mult = std::mem::take(&mut prison.charges) as i32;
    if mult > 0 {
        prison.add_stats(bought.attack * mult, bought.health * mult);
    }
}
