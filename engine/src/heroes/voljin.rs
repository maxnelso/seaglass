//! `Vol'jin` (`BG20_HERO_201`) — Armor 17 (Duos 15).
//!
//! Hero Power: `Spirit Swap` (`BG20_HERO_201p`), 0 Gold: Choose 2 minions. They gain each other's Attack until next turn.
//!
//! Active, `BoardOrShopMinion` (`0..7` friendly board, `7..14` Tavern minion), once per turn:
//! usable when at least 2 minions exist across `board` and `shop`. The first `HeroPower` picks a
//! minion (`armed = target + 1`) and restores `uses_left = 1` without spending Gold; the second
//! picks a different minion, clears `armed`, and gives each minion the other's current Attack
//! until next turn (`Unit::add_temp_attack`).

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 71463;
pub const POWER: CardId = 71464;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(spirit_swap)
            .on_hero_power_usable(usable)
            .on_hero_power_target(valid_target)
            .on_hero_turn_reset(|hero| hero.power.armed = 0)
            .with_activate_target(ActivateTargetKind::BoardOrShopMinion),
    )]
}

/// Usable when at least 2 minions exist across `board` and `shop`.
fn usable(state: &TavernState) -> bool {
    let shop_minions = state.shop.iter().filter(|u| !u.is_spell).count();
    state.board.len() + shop_minions >= 2
}

/// The second pick must be a different minion from the first (`armed = first + 1`).
fn valid_target(state: &TavernState, target: usize) -> bool {
    let armed = super::active_power(state).map_or(0, |p| p.armed);
    if armed == 0 {
        return true;
    }
    let first = (armed - 1) as usize;
    if minion_at(state, first).is_none() {
        return true;
    }
    target != first
}

/// Read the minion at `BoardOrShopMinion` target `t`.
fn minion_at(state: &TavernState, t: usize) -> Option<&Unit> {
    if t < 7 {
        state.board.get(t)
    } else {
        state.shop.get(t - 7).filter(|u| !u.is_spell)
    }
}

/// Mutably borrow the minion at `BoardOrShopMinion` target `t`.
fn minion_at_mut(state: &mut TavernState, t: usize) -> Option<&mut Unit> {
    if t < 7 {
        state.board.get_mut(t)
    } else {
        state.shop.get_mut(t - 7).filter(|u| !u.is_spell)
    }
}

/// Using it: first pick records `armed` and keeps `uses_left = 1`; second pick swaps Attack gains.
fn spirit_swap(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    let Some(t) = target else {
        return;
    };
    let armed = power_mut(&mut state.auras).map_or(0, |p| p.armed);
    if armed == 0 || minion_at(state, (armed - 1) as usize).is_none() {
        if let Some(power) = power_mut(&mut state.auras) {
            power.armed = (t + 1) as u32;
            power.uses_left = 1;
        }
        return;
    }
    let first = (armed - 1) as usize;
    if let Some(power) = power_mut(&mut state.auras) {
        power.armed = 0;
    }
    if first == t {
        return;
    }
    let (Some(atk_a), Some(atk_b)) = (
        minion_at(state, first).map(|u| u.attack.max(0)),
        minion_at(state, t).map(|u| u.attack.max(0)),
    ) else {
        return;
    };
    if let Some(a) = minion_at_mut(state, first) {
        a.add_temp_attack(atk_b);
    }
    if let Some(b) = minion_at_mut(state, t) {
        b.add_temp_attack(atk_a);
    }
}
