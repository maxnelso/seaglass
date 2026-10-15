//! `Rakanishu` (`TB_BaconShop_HERO_75`) — Armor 10 (Duos 10).
//!
//! Hero Power: `Tavern Lighting` (`TB_BaconShop_HP_085t`), 0 Gold: Your Tavern spells give an extra +1/+1. At the start of every 3 turns, improve this. (3 turns left!)
//!
//! Passive. When the player gets the hero (`hero_chosen`), spell power +1/+1 (`spell_bonus_atk` /
//! `spell_bonus_hp`). Every 3rd turn start (`hero_turn_start`, `counter`, counting the first:
//! engine decision) adds +1/+1 more.

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 64400;
pub const POWER: CardId = 122960;

/// Turns per improvement.
const TURNS: i32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_chosen(tavern_lighting)
            .on_hero_turn_start(turn_start),
    )]
}

/// Tavern spells give +1/+1 more, from now on.
fn tavern_lighting(state: &mut TavernState) {
    state.auras.spell_bonus_atk += 1;
    state.auras.spell_bonus_hp += 1;
}

/// Start of turn: every 3rd improves it.
fn turn_start(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter >= TURNS {
        power.counter -= TURNS;
        power.stacks += 1;
        tavern_lighting(state);
    }
}
