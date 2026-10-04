//! `Living Prison` (`BG36_180`) — Tier 4 Elemental (`4/5`).
//!
//! Activate (1): Gain (`double` if Golden) the stats of the next minion you buy this turn.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 438;
pub const ACTIVATE_COST: u32 = 1;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Living Prison", 4, 5, 4)
        .with_tribe(Tribe::Elemental)
        .with_activate_cost(ACTIVATE_COST)
        .on_activate(|state, source_pos, _, _, _| on_activate(state, source_pos))
}

pub fn on_activate(state: &mut TavernState, source_pos: usize) {
    if source_pos < state.board.len() {
        let mult = if state.board[source_pos].is_golden { 2 } else { 1 };
        state.board[source_pos].living_prison_stacks += mult;
    }
}
