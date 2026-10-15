//! `Triple Reward` (`TB_BaconShop_Triples_01`) — token spell (`0` Gold).
//!
//! Discover a minion of its Tier, locked in when it was received (`counter`: the receiver's Tavern
//! Tier + 1, at most 6; a card without one uses the Tier when it is cast):
//! `TavernState::offer_triple_reward_of` (`docs/tavern.md` §5.2). Not a Tavern spell, and never in
//! the shop. A Golden played from the hand gives it, and Hero Powers do (Guff Runetotem, ...). A
//! teammate's gift (Cho, Gall) arrives without one and locks in the receiver's as it arrives.

use super::spell;
use crate::cards::{CardFlags, CardHooks};
use crate::model::Unit;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    spell(cast)
        .on_received(lock_tier)
        .with_flags(CardFlags::NOT_IN_POOL | CardFlags::NOT_TAVERN_SPELL)
}

/// A teammate's gift without a locked-in Tier locks in the receiver's as it arrives.
fn lock_tier(card: &mut Unit, tavern_tier: u32) {
    if card.counter == 0 {
        card.counter = (tavern_tier + 1).min(6) as i32;
    }
}

/// `Triple Reward`: Discover a minion of its Tier.
pub fn cast(state: &mut TavernState, card: &Unit, _: usize, pool: &mut CardPool, rng: &mut Rng) {
    let tier = match card.counter {
        locked @ 1.. => locked as u32,
        _ => (state.tavern_tier + 1).min(6),
    };
    state.offer_triple_reward_of(tier, pool, rng);
}
