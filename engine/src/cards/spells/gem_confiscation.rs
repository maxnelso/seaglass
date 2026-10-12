//! `Gem Confiscation` (`BG28_698`) — Tier 4 token spell (`1` Gold).
//!
//! Play 3 Blood Gems on a minion and steal all Blood Gems from its neighbors.
//!
//! Its Blood Gems add nothing to its own cast, which counts once (`Felboar`; `docs/tavern.md`
//! §7.4).

use super::targeted_minion;
use crate::cards::{CardFlags, CardHooks};
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_minion(cast).with_flags(CardFlags::NOT_IN_POOL)
}

/// `Gem Confiscation`: play 3 Blood Gems on a minion and steal its neighbors' Blood Gems (its
/// neighbors on the board, or in the Tavern for a minion there).
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    target: MinionSlot,
    _: &mut CardPool,
    rng: &mut Rng,
) {
    let (zone, board_pos) = match target {
        MinionSlot::Board(pos) => (&mut state.board, pos),
        MinionSlot::Shop(idx) => (&mut state.shop, idx),
    };
    if board_pos < zone.len() {
        zone[board_pos].play_blood_gems(3, &state.auras);
        let mut neighbor_indices = Vec::new();
        if board_pos > 0 {
            neighbor_indices.push(board_pos - 1);
        }
        if board_pos + 1 < zone.len() {
            neighbor_indices.push(board_pos + 1);
        }
        let mut total_stolen_gems = 0u32;
        let mut total_stolen_atk = 0i32;
        let mut total_stolen_hp = 0i32;
        for n_idx in neighbor_indices {
            let neighbor = &mut zone[n_idx];
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
            let target = &mut zone[board_pos];
            target.blood_gems_played += total_stolen_gems;
            target.blood_gem_stats_applied.0 += total_stolen_atk;
            target.blood_gem_stats_applied.1 += total_stolen_hp;
            target.add_stats(total_stolen_atk, total_stolen_hp);
        }
        crate::cards::resolve_pending_effects(&mut state.board, &state.auras, rng);
    }
}
