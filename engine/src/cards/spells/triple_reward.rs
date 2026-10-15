//! `Triple Reward` (`TB_BaconShop_Triples_01`) — token spell (`0` Gold).
//!
//! Discover a minion of the next Tier: the Discover of a Golden's Triple Reward, with the Tavern
//! Tier when it is cast (`TavernState::offer_triple_reward`; `docs/tavern.md` §5.2). Not a Tavern
//! spell, and never in the shop. Hero Powers give it (Guff Runetotem).

use super::spell;
use crate::cards::{CardFlags, CardHooks};
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast).with_flags(CardFlags::NOT_IN_POOL | CardFlags::NOT_TAVERN_SPELL)
}

/// `Triple Reward`: Discover a minion of the next Tier.
pub fn cast(state: &mut TavernState, _: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    state.offer_triple_reward(pool, rng);
}
