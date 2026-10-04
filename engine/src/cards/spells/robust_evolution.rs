//! `Robust Evolution` — Tier 3 Tavern spell (`1` Gold).
//!
//! Transform a minion into a random minion of the next Tier, keeping its stats.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Robust Evolution`: transform a minion into a random minion of the next Tier, keeping its stats.
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if board_pos < state.board.len() {
        let old = state.board[board_pos].clone();
        let target_tier = (old.tavern_tier + 1).min(6);
        let mut opts = pool.draw_discover_options(target_tier, 1, rng);
        if let Some(mut evolved) = opts.pop() {
            pool.return_unit(&old);
            evolved.attack = old.attack;
            evolved.health = old.health;
            evolved.max_attack = old.attack.max(evolved.base_attack);
            evolved.max_health = old.health.max(evolved.base_health);
            crate::cards::check_stat_thresholds(&mut evolved);
            state.board[board_pos] = evolved;
            state.sync_all_auras();
        }
    }
}
