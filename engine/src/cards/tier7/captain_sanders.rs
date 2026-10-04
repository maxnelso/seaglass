//! `Captain Sanders` (`BG25_034`) — Tier 7 Pirate (`9/9`).
//!
//! Battlecry: Make a friendly minion from Tier 6 or below Golden
//! (two friendly minions from Tier 6 or below if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 701;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Captain Sanders", 9, 9, 7).with_tribe(Tribe::Pirate)
}

fn is_eligible(u: &Unit) -> bool {
    u.tavern_tier <= 6 && !u.is_golden && u.card_id != ID
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, board_pos: usize) {
    let count = if unit.is_golden { 2 } else { 1 };
    let mut made_any = false;
    for _ in 0..count {
        let target_idx = if board_pos < state.board.len() && is_eligible(&state.board[board_pos]) {
            Some(board_pos)
        } else if board_pos > 0
            && board_pos - 1 < state.board.len()
            && is_eligible(&state.board[board_pos - 1])
        {
            Some(board_pos - 1)
        } else {
            state.board.iter().position(is_eligible)
        };
        if let Some(idx) = target_idx {
            state.board[idx].make_golden();
            made_any = true;
        } else {
            break;
        }
    }
    if made_any {
        state.sync_all_auras();
    }
}
