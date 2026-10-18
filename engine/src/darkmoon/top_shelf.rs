//! `Top Shelf` (`BGS_Treasures_020`) — Tier 3 Darkmoon Prize.
//!
//! Discover a minion from a Tier higher (up to Tier 7).
//!
//! A Discover of 3 minions of exactly Tier `min(Tier + 1, 7)` from the pool, as a `Triple
//! Reward`'s (`TavernState::offer_triple_reward_of`).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65265;

/// The highest Tier it Discovers.
const MAX_TIER: u32 = 7;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    let tier = (state.tavern_tier + 1).min(MAX_TIER);
    state.offer_triple_reward_of(tier, pool, rng);
}
