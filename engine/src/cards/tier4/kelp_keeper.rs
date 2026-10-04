//! `Kelp Keeper` (`BG36_701`) — Tier 4 Murloc (`5/5`).
//!
//! Activate (1): Trigger a friendly minion's Battlecry (`twice` if Golden).

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 436;
pub const ACTIVATE_COST: u32 = 1;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Kelp Keeper", 5, 5, 4)
        .with_tribe(Tribe::Murloc)
        .with_activate_cost(ACTIVATE_COST)
}

pub fn on_activate(
    state: &mut TavernState,
    source_pos: usize,
    target_pos: Option<usize>,
    pool: &mut CardPool,
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
        cards::trigger_board_battlecry(state, t_pos, pool, rng);
    }
}
