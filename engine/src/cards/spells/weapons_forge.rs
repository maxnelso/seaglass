//! `Weapons Forge` — Tier 4 Tavern spell (`2` Gold).
//!
//! Get 3 `Pointy Arrow`s.

use super::spell;
use crate::cards::{tokens, CardHooks};
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
}

/// `Weapons Forge`: get 3 `Pointy Arrow`s.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    for _ in 0..3 {
        state.add_to_hand(tokens::make_pointy_arrow());
    }
}
