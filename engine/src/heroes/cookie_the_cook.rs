//! `Cookie the Cook` (`BG21_HERO_020`) — Armor 8 (Duos 6).
//!
//! Hero Power: `Stir the Pot` (`BG21_HERO_020p`), 0 Gold: Throw a minion in your pot. When you've gathered 3, Discover from their types. (3 left!)
//!
//! Active, targets a friendly or Tavern minion with a minion type (`ActivateTargetKind::BoardOrShopMinion`,
//! `docs/heroes.md` §11.2) without removing or destroying it. The first two throws record the
//! minion's `tribe` in `stacks` and increment `counter`; the 3rd throw (which requires room in the
//! hand) resets `counter` and `stacks` and offers a Discover of 3 distinct minions from the pool
//! (of the player's Tier or lower), one matching each of the 3 gathered types.

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::heroes::{active_power, hand_has_room, power_mut};
use crate::model::{CardId, HeroId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 77516;
pub const POWER: CardId = 77434;

const ALL_TRIBES: [Tribe; 22] = [
    Tribe::None,
    Tribe::Aberration,
    Tribe::Beast,
    Tribe::Demon,
    Tribe::Dragon,
    Tribe::Elemental,
    Tribe::Mech,
    Tribe::Murloc,
    Tribe::Pirate,
    Tribe::Quilboar,
    Tribe::Undead,
    Tribe::UndeadMech,
    Tribe::DragonPirate,
    Tribe::BeastPirate,
    Tribe::ElementalDemon,
    Tribe::MechMurloc,
    Tribe::DemonDragon,
    Tribe::DemonQuilboar,
    Tribe::All,
    Tribe::Naga,
    Tribe::DragonNaga,
    Tribe::DemonNaga,
];

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

fn usable(state: &TavernState) -> bool {
    active_power(state).is_some_and(|p| p.counter < 2 || hand_has_room(state))
}

fn valid_target(state: &TavernState, target: usize) -> bool {
    slot_unit(state, target).is_some_and(|u| u.tribe != Tribe::None)
}

fn encode_tribe(tribe: Tribe) -> i32 {
    ALL_TRIBES.iter().position(|&t| t == tribe).unwrap_or(0) as i32
}

fn decode_tribe(bits: i32) -> Tribe {
    ALL_TRIBES
        .get((bits & 0xFF) as usize)
        .copied()
        .unwrap_or(Tribe::None)
}

fn stir_the_pot(
    state: &mut TavernState,
    target: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(tribe) = target
        .and_then(|t| slot_unit(state, t))
        .map(|u| u.tribe)
        .filter(|&t| t != Tribe::None)
    else {
        return;
    };
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    if power.counter < 2 {
        let shift = 8 * power.counter.max(0) as usize;
        power.stacks |= encode_tribe(tribe) << shift;
        power.counter += 1;
        return;
    }
    let t0 = decode_tribe(power.stacks);
    let t1 = decode_tribe(power.stacks >> 8);
    let t2 = tribe;
    power.counter = 0;
    power.stacks = 0;
    let options = pool.draw_discover_by_tribes(&[t0, t1, t2], state.tavern_tier, rng);
    state.push_discover(options);
}
