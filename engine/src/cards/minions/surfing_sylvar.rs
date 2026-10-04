//! `Surfing Sylvar` (`BG32_235`) — Tier 2 Pirate (`1/2`).
//! At the end of your turn, give adjacent minions `+1` (`+2` if Golden) Attack.
//! Repeat for each friendly Golden minion.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 229;
pub const NAME: &str = "Surfing Sylvar";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 2, 2)
        .with_tribe(Tribe::Pirate)
        .on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let len = state.board.len();
    let golden_count = state.board.iter().filter(|u| u.is_golden).count() as i32;
    let repeats = 1 + golden_count;
    let atk_per_repeat = if state.board[self_idx].is_golden {
        2
    } else {
        1
    };
    let total_atk = atk_per_repeat * repeats;
    if self_idx > 0 {
        state.board[self_idx - 1].add_stats(total_atk, 0);
    }
    if self_idx + 1 < len {
        state.board[self_idx + 1].add_stats(total_atk, 0);
    }
}
