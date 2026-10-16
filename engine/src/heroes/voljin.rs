//! `Vol'jin` (`BG20_HERO_201`) — Armor 17 (Duos 15).
//!
//! Hero Power: `Spirit Swap` (`BG20_HERO_201p`), 0 Gold: Choose 2 minions. They gain each other's Attack until next turn.
//!
//! Active, `BoardOrShopMinion` (`0..7` friendly board, `7..14` Tavern minion), once per turn:
//! usable when at least 2 minions exist across `board` and `shop`. The first `HeroPower` picks a
//! minion (`picked`, by the id `heroes::identify_minion` gives it, so the pick follows the minion
//! when the board or the shop changes) and restores `uses_left = 1` without spending Gold; the
//! second picks a different minion, clears `picked`, and gives each minion the other's current
//! Attack until next turn (`Unit::add_temp_attack`). A pick whose minion left the board and the
//! shop has dropped: the next use is a first pick again.

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::heroes::{identify_minion, minion_target, power_mut};
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
            .on_hero_turn_reset(|hero| hero.power.picked = None)
            .with_activate_target(ActivateTargetKind::BoardOrShopMinion),
    )]
}

/// Usable when at least 2 minions exist across `board` and `shop`.
fn usable(state: &TavernState) -> bool {
    let shop_minions = state.shop.iter().filter(|u| !u.is_spell).count();
    state.board.len() + shop_minions >= 2
}

/// The current target of the first pick, if it is still on the board or in the shop.
fn first_pick(state: &TavernState) -> Option<usize> {
    let id = super::active_power(state)?.picked?;
    minion_target(state, id)
}

/// The second pick must be a different minion from the first.
fn valid_target(state: &TavernState, target: usize) -> bool {
    first_pick(state) != Some(target)
}

/// Mutably borrow the minion at `BoardOrShopMinion` target `t`.
fn minion_at_mut(state: &mut TavernState, t: usize) -> Option<&mut Unit> {
    if t < 7 {
        state.board.get_mut(t)
    } else {
        state.shop.get_mut(t - 7).filter(|u| !u.is_spell)
    }
}

/// Using it: the first pick is remembered and keeps `uses_left = 1`; the second swaps Attack.
fn spirit_swap(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    let Some(t) = target else {
        return;
    };
    let Some(first) = first_pick(state) else {
        let id = identify_minion(state, t);
        if let Some(power) = power_mut(&mut state.auras) {
            power.picked = id;
            power.uses_left = 1;
        }
        return;
    };
    if let Some(power) = power_mut(&mut state.auras) {
        power.picked = None;
    }
    if first == t {
        return;
    }
    let attack = |state: &mut TavernState, t| minion_at_mut(state, t).map(|u| u.attack.max(0));
    let (Some(atk_a), Some(atk_b)) = (attack(state, first), attack(state, t)) else {
        return;
    };
    if let Some(a) = minion_at_mut(state, first) {
        a.add_temp_attack(atk_b);
    }
    if let Some(b) = minion_at_mut(state, t) {
        b.add_temp_attack(atk_a);
    }
}
