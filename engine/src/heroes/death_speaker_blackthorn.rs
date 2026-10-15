//! `Death Speaker Blackthorn` (`BG20_HERO_103`) — Armor 18 (Duos 16).
//!
//! Hero Power: `Bloodbound` (`BG20_HERO_103p`), 1 Gold: Get 2 Blood Gems. (Twice per turn.)
//!
//! Active, untargeted, twice per turn: the first use of a turn gives the use back (`counter`
//! counts the turn's uses; `hero_turn_reset` clears it). Each use adds 2 `Blood Gem`s to the hand.
//! It needs room in the hand for one (engine decision); a Gem the hand has no room for is lost.

use crate::cards::{tokens, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, HeroState};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 71458;
pub const POWER: CardId = 71459;

/// Uses per turn.
const USES: i32 = 2;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(bloodbound)
            .on_hero_power_usable(super::hand_has_room)
            .on_hero_turn_reset(new_turn),
    )]
}

/// Using it: 2 Blood Gems, and the turn's second use.
fn bloodbound(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.counter += 1;
        if power.counter < USES {
            power.uses_left += 1;
        }
    }
    for _ in 0..2 {
        state.add_to_hand(tokens::make_blood_gem());
    }
}

/// Start of turn: no use counted yet.
fn new_turn(hero: &mut HeroState) {
    hero.power.counter = 0;
}
