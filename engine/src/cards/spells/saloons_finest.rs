//! `Saloon's Finest` — Tier 5 Tavern spell (`2` Gold).
//!
//! Replace the Tavern with Tavern spells (unfreezing it): each card in the Tavern becomes a random
//! Tavern spell, so 4 minions and a spell become 5 spells (and an empty slot stays empty). Frozen,
//! the spells stay next turn and each slot bought from refills with a minion (`docs/tavern.md`
//! §4).

use super::{draw_random_tavern_spell, spell};
use crate::cards::CardHooks;
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Saloon's Finest`: replace each card in the Tavern with a random Tavern spell (unfreezing it).
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    state.is_frozen = false;
    let cards = state.shop.len();
    for old in state.shop.drain(..) {
        pool.return_unit(&old);
    }
    for _ in 0..cards {
        state
            .shop
            .push(draw_random_tavern_spell(state.tavern_tier, rng));
    }
}
