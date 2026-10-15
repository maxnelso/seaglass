//! `Alexstrasza` (`TB_BaconShop_HERO_56`) — Armor 8 (Duos 6).
//!
//! Hero Power: `Queen of Dragons` (`TB_BaconShop_HP_064`), 1 Gold: Discover a Dragon. (Unlocks at Tier 4.)
//!
//! Active, untargeted, once per turn, from Tavern Tier 4 on: Discovers among 3 Dragons of the
//! player's Tier or lower from the pool; it needs room in the hand.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 61488;
pub const POWER: CardId = 61517;

/// The Tavern Tier it unlocks at.
const UNLOCK_TIER: u32 = 4;

/// Discover options.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(queen_of_dragons)
            .on_hero_power_usable(|state| {
                state.tavern_tier >= UNLOCK_TIER && super::hand_has_room(state)
            }),
    )]
}

/// Using it: Discover a Dragon.
fn queen_of_dragons(state: &mut TavernState, _: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    let options = pool.draw_discover_by_tribe(Tribe::Dragon, state.tavern_tier, OPTIONS, rng);
    state.push_discover(options);
}
