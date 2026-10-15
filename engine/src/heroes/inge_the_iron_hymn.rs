//! `Inge, the Iron Hymn` (`BG26_HERO_102`) — Armor 10 (Duos 8).
//!
//! Hero Power: `Major Hymn` (`BG26_HERO_102p`), 0 Gold: Twice per turn, give a minion Attack equal to your Tier. (Swaps to Health next turn!)
//!
//! Active, a friendly minion, twice per turn (the first use of a turn gives the use back, as
//! Blackthorn's): the minion gains Attack equal to the Tavern Tier on odd turns (and before the
//! first), Health on even turns (`armed`: 1 for Health, set at each turn start; engine decision).

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 103500;
pub const POWER: CardId = 103501;

/// Uses per turn.
const USES: i32 = 2;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(major_hymn)
            .on_hero_turn_start(turn_start)
            .with_activate_target(ActivateTargetKind::BoardAny),
    )]
}

/// Using it: Attack (or Health) equal to the Tier; the turn's second use.
fn major_hymn(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    let tier = state.tavern_tier as i32;
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter < USES {
        power.uses_left += 1;
    }
    let health = power.armed == 1;
    if let Some(unit) = target.and_then(|t| state.board.get_mut(t)) {
        if health {
            unit.add_stats(0, tier);
        } else {
            unit.add_stats(tier, 0);
        }
    }
}

/// Start of turn: no use counted yet; Health on even turns.
fn turn_start(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let even = state.turn.is_multiple_of(2);
    if let Some(power) = power_mut(&mut state.auras) {
        power.counter = 0;
        power.armed = u32::from(even);
    }
}
