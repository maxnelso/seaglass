//! `Elise Starseeker` (`TB_BaconShop_HERO_42`) — Armor 15 (Duos 13).
//!
//! Hero Power: `Lead Explorer` (`TB_BaconShop_HP_047`), 1 Gold: Discover a minion from your Tier. Costs (1) more after each use.
//!
//! Active, untargeted, once per turn: Discovers among 3 minions of exactly the player's Tier from
//! the pool (`CardPool::draw_discover_options`, as a Triple Reward); it needs room in the hand.
//! Each use makes it 1 more expensive for the rest of the game.

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 60213;
pub const POWER: CardId = 60217;

/// Discover options.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(lead_explorer)
            .on_hero_power_usable(super::hand_has_room),
    )]
}

/// Using it: Discover a minion of the player's Tier; 1 more expensive.
fn lead_explorer(state: &mut TavernState, _: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.cost += 1;
    }
    let options = pool.draw_discover_options(state.tavern_tier, OPTIONS, rng);
    state.push_discover(options);
}
