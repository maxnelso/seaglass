//! `Saloon's Finest` — Tier 5 Tavern spell (`2` Gold).
//!
//! Replace the Tavern with Tavern spells (unfreezing it).

use super::{draw_random_tavern_spell, spell};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{shop_capacity, CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Saloon's Finest`: replace the Tavern with Tavern spells (unfreezing it).
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    state.is_frozen = false;
    for old in state.shop.drain(..) {
        pool.return_unit(&old);
    }
    let cap = shop_capacity(state.tavern_tier);
    for _ in 0..cap {
        state
            .shop
            .push(draw_random_tavern_spell(state.tavern_tier, rng));
    }
}
