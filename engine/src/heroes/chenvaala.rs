//! `Chenvaala` (`TB_BaconShop_HERO_78`) — Armor 15 (Duos 13).
//!
//! Hero Power: `Avalanche` (`TB_BaconShop_HP_088`), 0 Gold: After you play 3 Elementals, reduce the Cost of upgrading the Tavern by (3).
//!
//! Passive. Each Elemental the player plays from hand counts (`hero_after_play`, `counter`, across
//! turns); every 3rd reduces the Cost of upgrading the Tavern by 3 (never below 0).

use crate::cards::{CardHooks, Played};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 64475;
pub const POWER: CardId = 64476;

/// Elementals to play per discount.
const ELEMENTALS: i32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_after_play(after_play))]
}

/// After an Elemental is played: every 3rd, the upgrade costs 3 less.
fn after_play(state: &mut TavernState, played: &Played, _: &mut CardPool, _: &mut Rng) {
    if !played.from_hand || !played.tribe.matches(Tribe::Elemental) {
        return;
    }
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter >= ELEMENTALS {
        power.counter -= ELEMENTALS;
        state.upgrade_cost = state.upgrade_cost.saturating_sub(3);
    }
}
