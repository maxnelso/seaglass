//! `Captain Sanders` (`BG25_034`) — Tier 7 Pirate (`9/9`).
//!
//! Battlecry: Make a friendly minion from Tier 6 or below Golden.
//!
//! A Golden Captain Sanders does nothing different: it makes one minion Golden too.

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 701;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Captain Sanders", 9, 9, 7)
        .with_tribe(Tribe::Pirate)
        .on_battlecry_target(|state, _, board_pos| pick_target(state, board_pos))
        .on_battlecry(|state, _, _, _, _| on_battlecry(state))
}

fn is_eligible(u: &Unit) -> bool {
    u.tavern_tier <= 6 && !u.is_golden && u.card_id != ID
}

/// The eligible friendly minion at `board_pos` (where this is played), else the one to its left,
/// else the first one.
fn pick_target(state: &TavernState, board_pos: usize) -> Option<usize> {
    if board_pos < state.board.len() && is_eligible(&state.board[board_pos]) {
        Some(board_pos)
    } else if board_pos > 0
        && board_pos - 1 < state.board.len()
        && is_eligible(&state.board[board_pos - 1])
    {
        Some(board_pos - 1)
    } else {
        state.board.iter().position(is_eligible)
    }
}

/// Make the target Golden. Nothing if the target is already Golden (a repeated trigger, see
/// `Brann Bronzebeard`).
pub fn on_battlecry(state: &mut TavernState) {
    let Some(target) =
        cards::battlecry_target_pos(&state.board).filter(|&t| is_eligible(&state.board[t]))
    else {
        return;
    };
    state.board[target].make_golden();
    state.sync_all_auras();
}
