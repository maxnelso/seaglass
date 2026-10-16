//! `Ambassador Faelin` (`BG22_HERO_201`) — Armor 14 (Duos 12).
//!
//! Hero Power: `Expedition Plans` (`BG22_HERO_201p`), 0 Gold: Skip your first turn. Discover minions from Tiers 6, 4, and 2 to get at those Tiers.
//!
//! Passive. `hero_skips_turn` on turn 1. At its start (`hero_turn_start`): three Discovers among 3
//! minions of exactly Tier 6, 4 and 2 from the pool, in that order, whose picks the Hero Power
//! keeps (`Unit::to_hero_power`, `stored`; the other options go back to the pool). Each kept
//! minion goes to the hand once the player's Tier reaches its Tier, waiting for room (confirmed):
//! at a turn start, after an Upgrade or after a minion is played (engine decision).
//!
//! Never offered by a Discover of Hero Powers (`CardFlags::NOT_DISCOVERABLE_POWER`): its skipped
//! turn would be past.

use crate::cards::{CardFlags, CardHooks, Played};
use crate::heroes::{hand_has_room, power_mut};
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 81569;
pub const POWER: CardId = 81570;

/// The Discovers' Tiers, in order.
const TIERS: [u32; 3] = [6, 4, 2];

/// Discover options.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .with_flags(CardFlags::NOT_DISCOVERABLE_POWER)
            .on_hero_skips_turn(|state| state.turn == 1)
            .on_hero_turn_start(turn_start)
            .on_hero_after_upgrade(|state, _, _| deliver(state))
            .on_hero_after_play(|state, _: &Played, _, _| deliver(state))
            .on_hero_option_chosen(keep),
    )]
}

/// Start of turn: the kept minions of the player's Tier or lower arrive; on the first turn, the
/// three Discovers.
fn turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    deliver(state);
    if state.turn != 1 {
        return;
    }
    for tier in TIERS {
        let mut options = pool.draw_discover_options(tier, OPTIONS, rng);
        for option in &mut options {
            option.to_hero_power = true;
        }
        state.push_discover(options);
    }
}

/// A pick is kept; the other options go back to the pool.
fn keep(
    state: &mut TavernState,
    mut chosen: Unit,
    unchosen: Vec<Unit>,
    pool: &mut CardPool,
    _: &mut Rng,
) {
    for option in &unchosen {
        pool.return_unit(option);
    }
    chosen.to_hero_power = false;
    if let Some(power) = power_mut(&mut state.auras) {
        power.stored.push(chosen);
    } else {
        pool.return_unit(&chosen);
    }
}

/// The kept minions of the player's Tier or lower go to the hand, in order, while it has room.
fn deliver(state: &mut TavernState) {
    let tier = state.tavern_tier;
    loop {
        let Some(power) = power_mut(&mut state.auras) else {
            return;
        };
        let Some(i) = power.stored.iter().position(|u| u.tavern_tier <= tier) else {
            return;
        };
        if !hand_has_room(state) {
            return;
        }
        let Some(power) = power_mut(&mut state.auras) else {
            return;
        };
        let minion = power.stored.remove(i);
        state.add_to_hand(minion);
    }
}
