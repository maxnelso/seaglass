//! `Surfing Sylvar` (`BG32_235`) — Tier 2 Pirate (`1/2`).
//! At the end of your turn, give adjacent minions `+1` (`+2` if Golden) Attack.
//! Repeat for each friendly Golden minion.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 229;
pub const NAME: &str = "Surfing Sylvar";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 2, 2).with_tribe(Tribe::Pirate)
}

pub fn on_end_turn(state: &mut TavernState) {
    let len = state.board.len();
    let golden_count = state.board.iter().filter(|u| u.is_golden).count() as i32;
    let repeats = 1 + golden_count;

    for idx in 0..len {
        if state.board[idx].card_id == ID {
            let atk_per_repeat = if state.board[idx].is_golden { 2 } else { 1 };
            let total_atk = atk_per_repeat * repeats;
            if idx > 0 {
                state.board[idx - 1].add_stats(total_atk, 0);
            }
            if idx + 1 < len {
                state.board[idx + 1].add_stats(total_atk, 0);
            }
        }
    }
}
