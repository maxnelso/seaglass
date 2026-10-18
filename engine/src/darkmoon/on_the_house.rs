//! `On the House` (`BGS_Treasures_012`) — Tier 2 Darkmoon Prize.
//!
//! Discover a minion of your Tier.
//!
//! A Discover of 3 minions of exactly the player's Tier from the pool, as a `Triple Reward`'s
//! (`TavernState::offer_triple_reward_of`).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65250;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    let tier = state.tavern_tier;
    state.offer_triple_reward_of(tier, pool, rng);
}
