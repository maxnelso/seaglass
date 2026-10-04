//! `Tavern Tempest` (`BGS_123`) — Tier 4 Elemental (`2/2`).
//!
//! Battlecry: Get a (`2` if Golden) random Elemental(s).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 456;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Tavern Tempest", 2, 2, 4)
        .with_tribe(Tribe::Elemental)
        .on_battlecry(|state, unit, _, pool, rng| on_battlecry(state, unit, pool, rng))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        if state.hand.len() >= 10 {
            break;
        }
        if let Some(drawn) =
            pool.draw_by_tribe(Tribe::Elemental, Some(ID), state.tavern_tier, rng)
        {
            state.add_to_hand(drawn);
        }
    }
}
