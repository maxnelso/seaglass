//! `Eonar's Favor` — Tier 4 Tavern spell (`2` Gold).
//!
//! Minions in the Tavern of a friendly minion's type have +3/+3 for the rest of the game.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::{Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Eonar's Favor`: minions in the Tavern of a friendly minion's type have +3/+3 for the rest of
/// the game.
pub fn cast(state: &mut TavernState, _: &Unit, board_pos: usize, _: &mut CardPool, _: &mut Rng) {
    if board_pos < state.board.len() {
        let target_tribe = state.board[board_pos].tribe;
        if target_tribe != Tribe::None {
            let (atk, hp) = state.auras.spell_stat_buff(3, 3);
            state.auras.tavern_tribe_buffs.push((target_tribe, atk, hp));
            for u in &mut state.shop {
                if !u.is_spell && u.tribe.matches(target_tribe) {
                    u.add_stats(atk, hp);
                }
            }
        }
    }
}
