//! `Enchanted Lasso` — Tier 1 Tavern spell (`2` Gold).
//!
//! Get a random minion from the Tavern.

use super::spell;
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Enchanted Lasso`: get a random minion from the Tavern.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    let shop_minions: Vec<usize> = state
        .shop
        .iter()
        .enumerate()
        .filter(|(_, u)| !u.is_spell)
        .map(|(i, _)| i)
        .collect();
    if !shop_minions.is_empty() && state.hand.len() < 10 {
        let pick = if shop_minions.len() == 1 {
            shop_minions[0]
        } else {
            shop_minions[rng.below(shop_minions.len())]
        };
        let stolen = state.shop.remove(pick);
        state.add_to_hand(stolen);
    }
}
