//! `Millificent Manastorm` (`TB_BaconShop_HERO_17`) — Armor 15 (Duos 13).
//!
//! Hero Power: `Tinker` (`TB_BaconShop_HP_015`), 1 Gold: Discover a Magnetic Mech. (Unlocks at Tier 4.)
//!
//! Active, untargeted, once per turn, from Tavern Tier 4 on: Discovers among 3 Magnetic Mechs (a
//! Mech or All minion with Magnetic) of the player's Tier or lower from the pool, as
//! Alexstrasza's; it needs room in the hand.

use crate::cards::{self, CardHooks};
use crate::model::{CardId, HeroId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 57946;
pub const POWER: CardId = 57949;

/// The Tavern Tier it unlocks at.
const UNLOCK_TIER: u32 = 4;

/// Discover options.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(tinker)
            .on_hero_power_usable(|state| {
                state.tavern_tier >= UNLOCK_TIER && super::hand_has_room(state)
            }),
    )]
}

/// A Magnetic Mech.
fn is_magnetic_mech(card_id: CardId) -> bool {
    cards::template(card_id).is_some_and(|t| t.magnetic && t.tribe.matches(Tribe::Mech))
}

/// Using it: Discover a Magnetic Mech.
fn tinker(state: &mut TavernState, _: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    let options = pool.draw_discover_filtered(state.tavern_tier, OPTIONS, is_magnetic_mech, rng);
    state.push_discover(options);
}
