//! `Channel the Devourer` — Tier 5 Tavern spell (`4` Gold).
//!
//! Sell a minion and give its stats to another random friendly minion.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Channel the Devourer`: sell a minion and give its stats to another random friendly minion.
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if board_pos < state.board.len() {
        let sold = state.board.remove(board_pos);
        let (sold_atk, sold_hp) = (sold.attack, sold.health);
        pool.return_unit(&sold);
        state.gold += 1;
        crate::cards::on_sell(state, &sold, pool, rng);
        if !state.board.is_empty() {
            let pick = if state.board.len() == 1 {
                0
            } else {
                rng.below(state.board.len())
            };
            let (atk, hp) = state.auras.spell_stat_buff(sold_atk, sold_hp);
            state.board[pick].add_stats(atk, hp);
        }
    }
}
