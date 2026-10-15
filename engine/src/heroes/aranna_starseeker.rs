//! `Aranna Starseeker` (`TB_BaconShop_HERO_59`) — Armor 12 (Duos 10).
//!
//! Hero Power: `Demon Hunter Training` (`TB_BaconShop_HP_065`), 0 Gold: After 14 friendly minions attack, the first minion you buy each turn is free. (14 left!)
//!
//! Passive. Friendly attacks count (`hero_friendly_attack`, `counter`, across combats); the 14th
//! unlocks it (`stacks` = 1). Unlocked, at the start of each turn (`hero_turn_start`) it makes the
//! next minion bought free (`next_minion_discount` covers the 3 Gold, `docs/tavern.md` §6), which
//! the first minion bought uses up.

use crate::cards::{BoardCtx, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 61910;
pub const POWER: CardId = 61915;

/// Friendly attacks to unlock it.
const ATTACKS: i32 = 14;

/// A minion's price: the discount that makes it free.
const MINION_PRICE: i32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_friendly_attack(friendly_attack)
            .on_hero_turn_start(turn_start),
    )]
}

/// A friendly minion attacked: the 14th attack unlocks it.
fn friendly_attack(ctx: &mut BoardCtx<'_>, _: UnitId) {
    let Some(power) = power_mut(ctx.auras).filter(|p| p.stacks == 0) else {
        return;
    };
    power.counter += 1;
    if power.counter >= ATTACKS {
        power.stacks = 1;
    }
}

/// Start of turn, unlocked: the first minion bought is free.
fn turn_start(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let unlocked = state
        .auras
        .hero
        .as_ref()
        .is_some_and(|h| h.power.stacks > 0);
    if unlocked {
        state.auras.next_minion_discount = state.auras.next_minion_discount.max(MINION_PRICE);
    }
}
