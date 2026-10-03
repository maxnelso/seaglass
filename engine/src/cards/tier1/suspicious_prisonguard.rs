//! `Suspicious Prisonguard` (`BG36_345`) — Tier 1 Neutral (`3/3`).
//! **Activate (1):** Give another minion `+3/+3` (`+6/+6` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 121;
pub const NAME: &str = "Suspicious Prisonguard";
pub const ACTIVATE_COST: u32 = 1;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 1)
        .with_tribe(Tribe::None)
        .with_activate_cost(ACTIVATE_COST)
}

pub fn on_activate(state: &mut TavernState, source_pos: usize, target_pos: Option<usize>) {
    if let Some(t_pos) = target_pos {
        if t_pos < state.board.len() && t_pos != source_pos {
            let mult = if state.board[source_pos].is_golden { 2 } else { 1 };
            state.board[t_pos].add_stats(3 * mult, 3 * mult);
        }
    }
}
