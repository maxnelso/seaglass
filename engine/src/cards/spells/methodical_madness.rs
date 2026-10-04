//! `Methodical Madness` — Tier 4 Tavern spell (`3` Gold).
//!
//! A minion consumes 2 random minions in the Tavern, gaining their stats and bonus keywords.

use super::targeted;
use crate::cards::CardHooks;
use crate::model::{Unit, BONUS_KEYWORDS};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted(cast)
}

/// `Methodical Madness`: a minion consumes 2 random minions in the Tavern, gaining their stats and
/// bonus keywords.
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    board_pos: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if board_pos < state.board.len() {
        for _ in 0..2 {
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
            let target = &mut state.board[board_pos];
            target.add_stats(consumed.attack, consumed.health);
            for kw in BONUS_KEYWORDS {
                if consumed.has_keyword(kw) {
                    target.apply_keyword(kw, false);
                }
            }
        }
    }
}
