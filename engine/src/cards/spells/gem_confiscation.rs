//! `Gem Confiscation` (`BG28_698`) — Tier 4 token spell (`1` Gold).
//!
//! Play 3 Blood Gems on a minion and steal all Blood Gems from its neighbors.

use super::targeted;
use crate::cards::{CardFlags, CardHooks};
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast).with_flags(CardFlags::NOT_IN_POOL)
}

/// `Gem Confiscation`: play 3 Blood Gems on a minion and steal its neighbors' Blood Gems.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, rng: &mut Rng) {
    if board_pos < state.board.len() {
        state.board[board_pos].play_blood_gems(3, &state.auras);
        let mut neighbor_indices = Vec::new();
        if board_pos > 0 {
            neighbor_indices.push(board_pos - 1);
        }
        if board_pos + 1 < state.board.len() {
            neighbor_indices.push(board_pos + 1);
        }
        let mut total_stolen_gems = 0u32;
        let mut total_stolen_atk = 0i32;
        let mut total_stolen_hp = 0i32;
        for n_idx in neighbor_indices {
            let neighbor = &mut state.board[n_idx];
            if neighbor.blood_gems_played > 0 {
                let (s_atk, s_hp) = neighbor.blood_gem_stats_applied;
                total_stolen_gems += neighbor.blood_gems_played;
                total_stolen_atk += s_atk;
                total_stolen_hp += s_hp;
                neighbor.attack = (neighbor.attack - s_atk).max(0);
                neighbor.health = (neighbor.health - s_hp).max(1);
                neighbor.blood_gems_played = 0;
                neighbor.blood_gem_stats_applied = (0, 0);
            }
        }
        if total_stolen_gems > 0 {
            let target = &mut state.board[board_pos];
            target.blood_gems_played += total_stolen_gems;
            target.blood_gem_stats_applied.0 += total_stolen_atk;
            target.blood_gem_stats_applied.1 += total_stolen_hp;
            target.add_stats(total_stolen_atk, total_stolen_hp);
        }
        crate::cards::resolve_pending_effects(&mut state.board, &state.auras, rng);
    }
}
