//! `Gacha Gift` (`BGS_Treasures_004`) — Tier 1 Darkmoon Prize.
//!
//! Discover a Tier 1 minion.
//!
//! A Discover of 3 Tier 1 minions from the pool, as a `Triple Reward`'s
//! (`TavernState::offer_triple_reward_of`).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65236;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    state.offer_triple_reward_of(1, pool, rng);
}
