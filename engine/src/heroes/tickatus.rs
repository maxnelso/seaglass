//! `Tickatus` (`TB_BaconShop_HERO_94`) — Armor 17 (Duos 15).
//!
//! Hero Power: `Prize Wall` (`TB_BaconShop_HP_106`), 0 Gold: Every 4 turns, Discover a Darkmoon Prize. (4 turns left!)
//!
//! Passive. At each turn start (`hero_turn_start`) the power's `counter` counts the turn; on the
//! 4th it starts again, `stacks` counts the Discover, and the player Discovers a prize
//! (`darkmoon::discover_prize`, `docs/darkmoon_prizes.md` §7.1): turns 4, 8, 12, ... for a hero
//! chosen before turn 1 (confirmed). The Tier follows the game turn: `min(4, max(1, turn / 4))`
//! (confirmed). A Prize Wall gained later counts its 4 turns from the turn it arrives (engine
//! decision).

use crate::cards::CardHooks;
use crate::darkmoon;
use crate::heroes::power_mut_for;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 67356;
pub const POWER: CardId = 67357;

/// Turns per prize.
const TURNS: i32 = 4;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_turn_start(turn_start))]
}

/// The prize Tier on game turn `turn` (confirmed).
pub fn prize_tier(turn: u32) -> u32 {
    (turn / TURNS as u32).clamp(1, darkmoon::MAX_TIER)
}

/// Start of turn: every 4th, a prize Discover.
fn turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let Some(power) = power_mut_for(&mut state.auras, POWER) else {
        return;
    };
    power.counter += 1;
    if power.counter < TURNS {
        return;
    }
    power.counter = 0;
    power.stacks += 1;
    let tier = prize_tier(state.turn);
    darkmoon::discover_prize(state, tier, pool, rng);
}
