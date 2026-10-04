//! `Ichoron the Protector` (`BG31_812`) — Tier 4 Elemental (`3/1`, Divine Shield).
//!
//! Divine Shield. Whenever you play an Elemental, give it Divine Shield until next turn (`permanently` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 433;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Ichoron the Protector", 3, 1, 4)
        .with_tribe(Tribe::Elemental)
        .with_keyword(Keyword::DivineShield)
}

pub fn after_play_minion(state: &mut TavernState, played_tribe: Tribe, board_pos: usize) {
    if !played_tribe.matches(Tribe::Elemental) || board_pos >= state.board.len() {
        return;
    }
    let mut has_normal = false;
    let mut has_golden = false;
    for (idx, u) in state.board.iter().enumerate() {
        if idx != board_pos && u.card_id == ID {
            if u.is_golden {
                has_golden = true;
            } else {
                has_normal = true;
            }
        }
    }
    if has_golden {
        state.board[board_pos].divine_shield = true;
        state.board[board_pos].temp_divine_shield = false;
    } else if has_normal && !state.board[board_pos].divine_shield {
        state.board[board_pos].divine_shield = true;
        state.board[board_pos].temp_divine_shield = true;
    }
}
