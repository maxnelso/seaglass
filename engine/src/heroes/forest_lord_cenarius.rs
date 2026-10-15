//! `Forest Lord Cenarius` (`BG32_HERO_001`) — Armor 13 (Duos 11).
//!
//! Hero Power: `Wisdom of Ancients` (`BG32_HERO_001p`), 3 Gold: Increase your maximum Gold by 1.
//!
//! Active, untargeted, once per turn. As `Strike Oil`: `max_gold` and `base_max_gold_bonus` go up
//! by 1 (above 10 too); the Gold itself doesn't.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 116920;
pub const POWER: CardId = 116921;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_power(wisdom))]
}

/// Using it: 1 more maximum Gold.
fn wisdom(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    state.auras.base_max_gold_bonus += 1;
    state.max_gold += 1;
}
