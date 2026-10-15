//! `Channel the Devourer` — Tier 5 Tavern spell (`4` Gold).
//!
//! Sell a minion and give its stats to another random friendly minion. The stats are copied as
//! they are: spell power (`spell_bonus_atk` / `spell_bonus_hp`) does not add to them. The sale is a
//! sale like the `Sell` action's (`TavernState::sell_minion`: the hero's and, in Duos, the
//! teammate's observers see it).

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
        let sold = state.sell_minion(board_pos, pool, rng);
        let (sold_atk, sold_hp) = (sold.attack, sold.health);
        if !state.board.is_empty() {
            let pick = if state.board.len() == 1 {
                0
            } else {
                rng.below(state.board.len())
            };
            state.board[pick].add_stats(sold_atk, sold_hp);
        }
    }
}
