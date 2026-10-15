//! `Snake Eyes` (`BG28_HERO_400`) — Armor 8 (Duos 6).
//!
//! Hero Power: `Lucky Roll` (`BG28_HERO_400p`), 1 Gold: Roll a 6-sided die. Gain that much Gold. (Cannot be used again for that many turns!)
//!
//! Active, untargeted: the player gains `1 + rng.below(6)` Gold. The roll is its cooldown
//! (`counter`): at each turn start while it is above 0 (`hero_turn_reset`), the power has no use
//! and the cooldown goes down by 1, so a 3 rolled on turn 5 makes it usable again on turn 9.

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, HeroState};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 105314;
pub const POWER: CardId = 105315;

/// The die's faces.
const FACES: usize = 6;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(lucky_roll)
            .on_hero_turn_reset(cool_down),
    )]
}

/// Using it: roll, gain that much Gold, and wait that many turns.
fn lucky_roll(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, rng: &mut Rng) {
    let roll = 1 + rng.below(FACES) as u32;
    state.gold += roll;
    if let Some(power) = power_mut(&mut state.auras) {
        power.counter = roll as i32;
    }
}

/// Start of turn: a turn of the cooldown passes, with no use.
fn cool_down(hero: &mut HeroState) {
    if hero.power.counter > 0 {
        hero.power.counter -= 1;
        hero.power.uses_left = 0;
    }
}
