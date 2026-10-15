//! `Skycap'n Kragg` (`TB_BaconShop_HERO_68`) — Armor 14 (Duos 12).
//!
//! Hero Power: `Piggy Bank` (`TB_BaconShop_HP_076`), 0 Gold: Gain 2 Gold. Increases by 1 each turn. (Once per game.)
//!
//! Active, untargeted, once per game (`stacks` is 1 once used): the player gains
//! `2 + (turns started - 1)` Gold (`counter` counts the turns started, `hero_turn_reset`; 2 before
//! the first turn).

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, HeroState};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 62268;
pub const POWER: CardId = 62269;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(piggy_bank)
            .on_hero_power_usable(super::unused_this_game)
            .on_hero_turn_reset(count_turn),
    )]
}

/// Using it: 2 Gold, +1 per turn since the first, once per game.
fn piggy_bank(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.stacks = 1;
    let gold = 2 + (power.counter - 1).max(0);
    state.gold += gold.unsigned_abs();
}

/// Start of turn: one more turn counted.
fn count_turn(hero: &mut HeroState) {
    hero.power.counter += 1;
}
