//! `Genn, Worgen King` (`BG35_HERO_001`) — Armor 7 (Duos 5).
//!
//! Hero Power: `King of Duality` (`BG35_HERO_001p`), 0 Gold: On Turn 4, Discover two Hero Powers to replace this. (3 turns left!)
//!
//! Passive. Counts down `counter` from 3 on turn 1 to 1 on turn 3; on turn 4, offers a first
//! Discover of 3 Hero Powers to replace `hero.power`, and once chosen, offers a second Discover of
//! 3 Hero Powers (excluding the first) to install in `hero.second_power`.

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 129684;
pub const POWER: CardId = 129685;

/// The turn it triggers on.
const TRIGGER_TURN: u32 = 4;

/// Discover options per Hero Power.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_chosen(chosen)
            .on_hero_turn_start(turn_start)
            .on_hero_option_chosen(choose_first_power),
    )]
}

/// Start of game: 3 turns left.
fn chosen(state: &mut TavernState) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.counter = 3;
    }
}

/// Start of turns 1–3: update countdown; on turn 4: Discover the first Hero Power.
fn turn_start(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    if state.turn < TRIGGER_TURN {
        if let Some(power) = power_mut(&mut state.auras) {
            power.counter = (TRIGGER_TURN - state.turn) as i32;
        }
    } else if state.turn == TRIGGER_TURN {
        if let Some(power) = power_mut(&mut state.auras) {
            power.counter = 0;
        }
        super::offer_power_discover(state, OPTIONS, None, false, false, rng);
    }
}

/// Choosing the first Hero Power installs it in `hero.power`, then immediately offers the second
/// Discover (options marked with `counter = 1` so `keep_chosen` installs the choice into
/// `hero.second_power`).
fn choose_first_power(
    state: &mut TavernState,
    chosen: Unit,
    _: Vec<Unit>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let first_id = chosen.card_id;
    super::install_power(state, first_id, false, pool, rng);
    super::offer_power_discover(state, OPTIONS, Some(first_id), false, true, rng);
}
