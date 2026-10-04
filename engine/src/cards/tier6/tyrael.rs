//! `Tyrael` (`BG36_356`) — Tier 6 Neutral (`10/10`).
//!
//! Activate (1): Set another minion's stats to `50/50` (`100/100` if Golden).

use crate::cards::{self, CardTemplate};
use crate::model::CardId;
use crate::tavern::TavernState;

pub const ID: CardId = 626;
pub const ACTIVATE_COST: u32 = 1;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Tyrael", 10, 10, 6).with_activate_cost(ACTIVATE_COST)
}

pub fn on_activate(state: &mut TavernState, source_pos: usize, target_pos: Option<usize>) {
    let Some(t_pos) = target_pos else {
        return;
    };
    if source_pos >= state.board.len() || t_pos >= state.board.len() || t_pos == source_pos {
        return;
    }
    let stat = if state.board[source_pos].is_golden {
        100
    } else {
        50
    };
    let target = &mut state.board[t_pos];
    target.attack = stat;
    target.health = stat;
    target.sync_max_stats();
    cards::check_stat_thresholds(target);
}
