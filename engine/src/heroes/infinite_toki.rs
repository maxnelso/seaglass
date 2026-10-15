//! `Infinite Toki` (`TB_BaconShop_HERO_28`) — Armor 15 (Duos 13).
//!
//! Hero Power: `Temporal Tavern` (`TB_BaconShop_HP_028`), 1 Gold: Refresh the Tavern. Include two minions from a Tier higher than yours.
//!
//! Active, untargeted, once per turn: a Refresh (`TavernState::refresh_shop_with_tier_minions`)
//! whose last 2 minions are of exactly the next Tier (Tier 6 at Tier 6; from the pool, falling
//! back to the highest Tier below with copies left; engine decision), with the after-Refresh
//! effects of any Refresh.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 58534;
pub const POWER: CardId = 58537;

/// Minions of the next Tier.
const HIGHER: usize = 2;

/// The highest Tavern Tier.
const MAX_TIER: u32 = 6;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_power(temporal_tavern))]
}

/// Using it: a Refresh with 2 minions of the next Tier.
fn temporal_tavern(state: &mut TavernState, _: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    let tier = (state.tavern_tier + 1).min(MAX_TIER);
    state.refresh_shop_with_tier_minions(tier, HIGHER, pool, rng);
}
