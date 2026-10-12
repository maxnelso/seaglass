//! `Methodical Madness` — Tier 4 Tavern spell (`3` Gold).
//!
//! A minion consumes 2 random minions in the Tavern, gaining their stats and bonus keywords.

use super::targeted_minion;
use crate::cards::CardHooks;
use crate::model::{Unit, BONUS_KEYWORDS};
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    targeted_minion(cast)
}

/// `Methodical Madness`: a minion consumes 2 random minions in the Tavern, gaining their stats and
/// bonus keywords (other ones than itself, for a minion in the Tavern).
pub fn cast(
    state: &mut TavernState,
    _: &Unit,
    mut target: MinionSlot,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if state.minion(target).is_some() {
        for _ in 0..2 {
            let shop_minions: Vec<usize> = state
                .shop
                .iter()
                .enumerate()
                .filter(|&(i, u)| !u.is_spell && target != MinionSlot::Shop(i))
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
            if let MinionSlot::Shop(idx) = &mut target {
                if pick < *idx {
                    *idx -= 1;
                }
            }
            let Some(target) = state.minion_mut(target) else {
                break;
            };
            target.add_stats(consumed.attack, consumed.health);
            for kw in BONUS_KEYWORDS {
                if consumed.has_keyword(kw) {
                    target.apply_keyword(kw, false);
                }
            }
        }
    }
}
