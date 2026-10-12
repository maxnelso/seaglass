//! `Maw Caster` (`BG32_340`) — Tier 4 Undead (`4/5`).
//!
//! Battlecry: Destroy a friendly Undead to Discover an (`2` if Golden) Undead.

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 441;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Maw Caster", 4, 5, 4)
        .with_tribe(Tribe::Undead)
        .on_battlecry_target(pick_target)
        .on_battlecry(|state, unit, _, pool, rng| on_battlecry(state, unit, pool, rng))
}

/// The Battlecry's target: the friendly Undead to the left of `board_pos` (where this is
/// played), else the one at `board_pos`, else the first one.
pub fn pick_target(state: &TavernState, _: &Unit, board_pos: usize) -> Option<usize> {
    if board_pos > 0
        && board_pos - 1 < state.board.len()
        && state.board[board_pos - 1].tribe.matches(Tribe::Undead)
    {
        Some(board_pos - 1)
    } else if board_pos < state.board.len() && state.board[board_pos].tribe.matches(Tribe::Undead) {
        Some(board_pos)
    } else {
        state
            .board
            .iter()
            .position(|u| u.tribe.matches(Tribe::Undead))
    }
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(idx) = cards::battlecry_target_pos(&state.board) {
        state.destroy_board_unit(idx, pool, rng);
        let count = if unit.is_golden { 2 } else { 1 };
        for _ in 0..count {
            let mut opts = pool.draw_discover_by_tribe(Tribe::Undead, state.tavern_tier, 3, rng);
            for opt in &mut opts {
                state.apply_global_unit_auras(opt);
            }
            if !opts.is_empty() {
                state.push_discover(opts);
            }
        }
    }
}
