//! `Kurtrus Ashfallen` (`BG20_HERO_280`) — Armor 16 (Duos 14).
//!
//! Hero Power: `Glaive Ricochet` (`BG20_HERO_280p5`), 0 Gold: Once per turn, after you buy 3 minions, get a plain copy of one of them. (3 left!)
//!
//! Passive. The minions bought this turn count (`hero_after_buy`, `counter`; the count starts
//! again each turn: engine decision) and are remembered (`stored`); the 3rd gives the player a
//! plain copy (`cards::plain_instance`) of one of the 3, at random (one `rng.below(3)`), once per
//! turn (`armed`).

use crate::cards::{self, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, HeroState, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 74631;
pub const POWER: CardId = 104875;

/// Minions to buy.
const BUYS: i32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_after_buy(after_buy)
            .on_hero_turn_reset(new_turn),
    )]
}

/// After a minion is bought: the 3rd of the turn gives a plain copy of one of them.
fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, rng: &mut Rng) {
    if bought.is_spell {
        return;
    }
    let Some(power) = power_mut(&mut state.auras).filter(|p| p.armed == 0) else {
        return;
    };
    power.counter += 1;
    power.stored.push(bought.clone());
    if power.counter < BUYS {
        return;
    }
    power.armed = 1;
    let bought = std::mem::take(&mut power.stored);
    let pick = rng.below(bought.len());
    if let Some(copy) = cards::plain_instance(&bought[pick]) {
        state.add_to_hand(copy);
    }
}

/// Start of turn: nothing bought yet.
fn new_turn(hero: &mut HeroState) {
    hero.power.counter = 0;
    hero.power.armed = 0;
    hero.power.stored.clear();
}
