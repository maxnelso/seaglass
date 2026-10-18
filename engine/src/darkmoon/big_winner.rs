//! `Big Winner!` (`BGS_Treasures_032`) — Tier 4 Darkmoon Prize.
//!
//! Discover a Darkmoon Prize from each of the previous Prize Tiers.
//!
//! Three prize Discovers (`super::discover_prize`): Tier 1, then 2, then 3, queued in that order
//! (engine decision: the order).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 66460;

/// The prize Tiers below its own.
const TIERS: [u32; 3] = [1, 2, 3];

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    for tier in TIERS {
        super::discover_prize(state, tier, pool, rng);
    }
}
