//! `Corrupted Cupcakes` — Tier 5 Tavern spell (`4` Gold).
//!
//! A friendly Demon consumes 3 random minions in the Tavern, gaining their stats.

use super::targeted_tribe;
use crate::cards::CardHooks;
use crate::model::{Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_tribe(Tribe::Demon, cast)
}

/// `Corrupted Cupcakes`: a friendly Demon consumes 3 random minions in the Tavern, gaining their
/// stats.
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if board_pos < state.board.len() && state.board[board_pos].tribe.matches(Tribe::Demon) {
        for _ in 0..3 {
            let shop_minions: Vec<usize> = state
                .shop
                .iter()
                .enumerate()
                .filter(|(_, u)| !u.is_spell)
                .map(|(i, _)| i)
                .collect();
            if shop_minions.is_empty() {
                break;
            }
            let pick = if shop_minions.len() == 1 {
                shop_minions[0]
            } else {
                shop_minions[rng.below(shop_minions.len())]
            };
            let consumed = state.shop.remove(pick);
            pool.return_unit(&consumed);
            state.board[board_pos].add_stats(consumed.attack, consumed.health);
        }
    }
}
