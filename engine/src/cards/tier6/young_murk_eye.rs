//! `Young Murk-Eye` (`BG22_403`) — Tier 6 Murloc (`8/5`).
//!
//! At the end of your turn, trigger the Battlecries of adjacent minions (`twice` if Golden).

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 632;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Young Murk-Eye", 8, 5, 6)
        .with_tribe(Tribe::Murloc)
        .on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, pool: &mut CardPool, rng: &mut Rng) {
    let repeats = if state.board[self_idx].is_golden {
        2
    } else {
        1
    };
    for _ in 0..repeats {
        if self_idx >= state.board.len() {
            break;
        }
        if self_idx > 0 {
            cards::trigger_board_battlecry(state, self_idx - 1, pool, rng);
        }
        if self_idx + 1 < state.board.len() {
            cards::trigger_board_battlecry(state, self_idx + 1, pool, rng);
        }
    }
}
