//! `Elite Navigator` (`BG32_231`) — Tier 5 Pirate (`5/5`).
//!
//! Battlecry: Make a friendly Pirate from Tier 4 or below Golden.
//!
//! A targeted Battlecry (`docs/tavern.md` §7.1): the target is chosen with the play position, like
//! `Captain Sanders`, so a repeat (`Brann Bronzebeard`) finds it Golden and does nothing. For now a
//! Golden Elite Navigator does the same (one Pirate).

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 514;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Elite Navigator", 5, 5, 5)
        .with_tribe(Tribe::Pirate)
        .on_battlecry_target(|state, _, board_pos| pick_target(state, board_pos))
        .on_battlecry(|state, _, _, _, _| on_battlecry(state))
}

fn is_eligible(u: &Unit) -> bool {
    u.tribe.matches(Tribe::Pirate) && u.tavern_tier <= 4 && !u.is_golden
}

/// The eligible friendly Pirate at `board_pos` (where this is played), else the one to its left,
/// else the first one.
fn pick_target(state: &TavernState, board_pos: usize) -> Option<usize> {
    let eligible = |pos: usize| state.board.get(pos).is_some_and(is_eligible);
    if eligible(board_pos) {
        Some(board_pos)
    } else if board_pos > 0 && eligible(board_pos - 1) {
        Some(board_pos - 1)
    } else {
        state.board.iter().position(is_eligible)
    }
}

/// Make the target Golden. Nothing if it is already Golden (a repeated trigger).
pub fn on_battlecry(state: &mut TavernState) {
    let Some(target) =
        cards::battlecry_target_pos(&state.board).filter(|&t| is_eligible(&state.board[t]))
    else {
        return;
    };
    state.board[target].make_golden();
    state.sync_all_auras();
}
