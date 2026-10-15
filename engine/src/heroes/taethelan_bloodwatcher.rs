//! `Tae'thelan Bloodwatcher` (`BG28_HERO_800`) — Armor 18 (Duos 16).
//!
//! Hero Power: `Reliquary Research` (`BG28_HERO_800p`), 0 Gold: Every third Tavern spell you buy costs (0). (2 left!)
//!
//! Passive. Each Tavern spell bought counts (`hero_after_buy`, `counter`, across turns); after the
//! 2nd, the next spell bought costs 0 (`next_spell_discount` covers its whole cost), and the 3rd
//! starts the count again.

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 105431;
pub const POWER: CardId = 105432;

/// Spells per free spell.
const SPELLS: i32 = 3;

/// A discount no spell costs more than.
const FREE: i32 = 100;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_after_buy(after_buy))]
}

/// After a spell is bought: after the 2nd, the next one is free.
fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    if !bought.is_spell {
        return;
    }
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter >= SPELLS {
        power.counter = 0;
    } else if power.counter == SPELLS - 1 {
        state.auras.next_spell_discount = state.auras.next_spell_discount.max(FREE);
    }
}
