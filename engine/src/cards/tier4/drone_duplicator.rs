//! `Drone Duplicator` (`BG36_506`) — Tier 4 Mech (`5/2`, Divine Shield).
//!
//! Divine Shield. Activate (1): The next Magnetization to this minion this turn happens an extra time (`2` extra times if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 416;
pub const ACTIVATE_COST: u32 = 1;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Drone Duplicator", 5, 2, 4)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::DivineShield)
        .with_activate_cost(ACTIVATE_COST)
        .on_activate(|state, source_pos, _, _, _| on_activate(state, source_pos))
        .on_reset_turn_charges(|u| u.charges = 0)
        .on_extra_magnetizations(|u| std::mem::take(&mut u.charges))
}

pub fn on_activate(state: &mut TavernState, source_pos: usize) {
    if source_pos < state.board.len() {
        let extra = if state.board[source_pos].is_golden { 2 } else { 1 };
        state.board[source_pos].charges += extra;
    }
}
