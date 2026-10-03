//! `Mechagnome Interpreter` (`BG31_177`) — Tier 2 Mech (`3/1`).
//! Whenever you play or **Magnetize** a Mech, give it `+3/+1` (`+6/+2` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 218;
pub const NAME: &str = "Mechagnome Interpreter";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 1, 2).with_tribe(Tribe::Mech)
}

pub fn after_play_or_magnetize_mech(
    state: &mut TavernState,
    played_tribe: Tribe,
    target_board_pos: usize,
    was_magnetized: bool,
) {
    if !played_tribe.matches(Tribe::Mech) || target_board_pos >= state.board.len() {
        return;
    }
    let total_mult: i32 = state
        .board
        .iter()
        .enumerate()
        .filter(|(idx, u)| {
            u.card_id == ID && (was_magnetized || *idx != target_board_pos)
        })
        .map(|(_, u)| if u.is_golden { 2 } else { 1 })
        .sum();

    if total_mult > 0 {
        state.board[target_board_pos].add_stats(3 * total_mult, total_mult);
    }
}
