//! `Sky-hatch Runaway` (`BG36_243`) — Tier 4 Dragon (`4/7`).
//!
//! Activate (1): Trigger a friendly minion's Rally (`twice` if Golden).

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 452;
pub const ACTIVATE_COST: u32 = 1;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Sky-hatch Runaway", 4, 7, 4)
        .with_tribe(Tribe::Dragon)
        .with_activate_cost(ACTIVATE_COST)
}

pub fn on_activate(
    state: &mut TavernState,
    source_pos: usize,
    target_pos: Option<usize>,
    rng: &mut Rng,
) {
    let Some(t_pos) = target_pos else {
        return;
    };
    if t_pos >= state.board.len() {
        return;
    }
    let repeats = if state.board[source_pos].is_golden { 2 } else { 1 };
    for _ in 0..repeats {
        if t_pos >= state.board.len() {
            break;
        }
        cards::trigger_tavern_rally(state, t_pos, rng);
    }
}
