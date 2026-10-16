//! `Cookie the Cook` (`BG21_HERO_020`) — Armor 8 (Duos 6).
//!
//! Hero Power: `Stir the Pot` (`BG21_HERO_020p`), 0 Gold: Throw a minion in your pot. When you've gathered 3, Discover from their types. (3 left!)
//!
//! Active, targets a friendly or Tavern minion with a minion type (`ActivateTargetKind::BoardOrShopMinion`,
//! `docs/heroes.md` §11.2) without removing or destroying it. The first two throws keep a plain
//! copy of the minion in the pot (`stored`) and increment `counter`; the 3rd throw (which requires
//! room in the hand) empties the pot and offers a Discover of 3 distinct minions from the pool (of
//! the player's Tier or lower), one matching each of the 3 gathered types.

use crate::cards::{self, ActivateTargetKind, CardHooks};
use crate::heroes::{active_power, hand_has_room, power_mut};
use crate::model::{CardId, HeroId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 77516;
pub const POWER: CardId = 77434;

/// The minions the pot gathers before its Discover.
const POT_SIZE: usize = 3;

pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .with_activate_target(ActivateTargetKind::BoardOrShopMinion)
            .on_hero_power(stir_the_pot)
            .on_hero_power_usable(usable)
            .on_hero_power_target(valid_target),
    )]
}

fn slot_unit(state: &TavernState, slot: usize) -> Option<&Unit> {
    if slot < 7 {
        state.board.get(slot)
    } else if slot < 14 {
        state.shop.get(slot - 7).filter(|u| !u.is_spell)
    } else {
        None
    }
}

/// The last throw needs room in the hand for the Discover.
fn usable(state: &TavernState) -> bool {
    active_power(state).is_some_and(|p| p.stored.len() + 1 < POT_SIZE || hand_has_room(state))
}

fn valid_target(state: &TavernState, target: usize) -> bool {
    slot_unit(state, target).is_some_and(|u| u.tribe != Tribe::None)
}

/// Using it: a plain copy of the minion goes in the pot; the 3rd Discovers from the 3 types.
fn stir_the_pot(
    state: &mut TavernState,
    target: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(thrown) = target
        .and_then(|t| slot_unit(state, t))
        .filter(|u| u.tribe != Tribe::None)
        .map(cards::instantiate_plain_copy)
    else {
        return;
    };
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.stored.push(thrown);
    power.counter = power.stored.len() as i32;
    if power.stored.len() < POT_SIZE {
        return;
    }
    let tribes: Vec<Tribe> = std::mem::take(&mut power.stored)
        .iter()
        .map(|u| u.tribe)
        .collect();
    power.counter = 0;
    let options = pool.draw_discover_by_tribes(&tribes, state.tavern_tier, rng);
    state.push_discover(options);
}
