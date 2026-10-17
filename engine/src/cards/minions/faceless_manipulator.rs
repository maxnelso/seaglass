//! `Faceless Manipulator` (`BG_EX1_564`) — Tier 1 Aberration (`3/3`), in no pool.
//! **Battlecry:** Choose a minion and become a copy of it.
//!
//! Only `Manipulator Portrait` gives it (`docs/trinkets.md` §11.9). The minion it copies is a
//! friendly one, picked as `Captain Sanders`' target: the minion where it is played (to its
//! right), else the one to its left, else the first one **(engine decision)**. It becomes an exact
//! copy (stats, keywords, enchantments, Golden or not) and keeps only its own identity; the
//! trinkets' marks stay on the original. A Golden Manipulator does the same.

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10806;
pub const NAME: &str = "Faceless Manipulator";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 1)
        .with_tribe(Tribe::Aberration)
        .on_battlecry_target(|state, _, board_pos| pick_target(state, board_pos))
        .on_battlecry(battlecry)
}

/// The friendly minion at `board_pos` (where this is played), else the one to its left, else the
/// first one.
fn pick_target(state: &TavernState, board_pos: usize) -> Option<usize> {
    if board_pos < state.board.len() {
        Some(board_pos)
    } else if board_pos > 0 && board_pos - 1 < state.board.len() {
        Some(board_pos - 1)
    } else if state.board.is_empty() {
        None
    } else {
        Some(0)
    }
}

/// Become a copy of the target.
pub fn battlecry(
    state: &mut TavernState,
    unit: &mut Unit,
    _: usize,
    _: &mut CardPool,
    _: &mut Rng,
) {
    let Some(target) = cards::battlecry_target_pos(&state.board) else {
        return;
    };
    let id = unit.id;
    *unit = state.board[target].clone();
    unit.id = id;
    unit.trinket_marks.clear();
}
