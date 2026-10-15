//! `Overlord Saurfang` (`BG20_HERO_102`) — Armor 15 (Duos 15).
//!
//! Hero Power: `For the Horde!` (`BG20_HERO_102p`), 0 Gold: Minions in the Tavern have +1/+1. Improves after you buy 3 minions. (3 left!)
//!
//! Passive. When the player gets the hero (`hero_chosen`), the Tavern's minions get +1/+1
//! (`tavern_all_atk` / `tavern_all_hp`, the minions in the shop too). Every 3rd minion bought
//! (counted across turns: `hero_after_buy`, `counter`) adds +1/+1 more.

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 71454;
pub const POWER: CardId = 71455;

/// Minions to buy per improvement.
const BUYS: i32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_chosen(for_the_horde)
            .on_hero_after_buy(after_buy),
    )]
}

/// The Tavern's minions gain +1/+1, from now on.
fn for_the_horde(state: &mut TavernState) {
    state.auras.tavern_all_atk += 1;
    state.auras.tavern_all_hp += 1;
    for unit in state.shop.iter_mut().filter(|u| !u.is_spell) {
        unit.add_stats(1, 1);
    }
}

/// After a card is bought: every 3rd minion improves it.
fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    if bought.is_spell {
        return;
    }
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter >= BUYS {
        power.counter -= BUYS;
        power.stacks += 1;
        for_the_horde(state);
    }
}
