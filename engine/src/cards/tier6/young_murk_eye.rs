//! `Young Murk-Eye` (`BG22_403`) — Tier 6 Murloc (`8/5`).
//!
//! At the end of your turn, trigger the Battlecries of adjacent minions (`twice` if Golden).

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 632;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Young Murk-Eye", 8, 5, 6).with_tribe(Tribe::Murloc)
}

pub fn on_end_turn(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let murk_eyes: Vec<(usize, u32)> = state
        .board
        .iter()
        .enumerate()
        .filter(|(_, u)| u.card_id == ID)
        .map(|(i, u)| (i, if u.is_golden { 2 } else { 1 }))
        .collect();
    for (pos, repeats) in murk_eyes {
        for _ in 0..repeats {
            if pos >= state.board.len() {
                break;
            }
            if pos > 0 {
                cards::trigger_board_battlecry(state, pos - 1, pool, rng);
            }
            if pos + 1 < state.board.len() {
                cards::trigger_board_battlecry(state, pos + 1, pool, rng);
            }
        }
    }
}
