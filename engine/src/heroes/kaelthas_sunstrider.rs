//! `Kael'thas Sunstrider` (`TB_BaconShop_HERO_60`) — Armor 18 (Duos 16).
//!
//! Hero Power: `Verdant Spheres` (`TB_BaconShop_HP_066`), 0 Gold: After you buy 3 minions, get a Tavern Coin.
//!
//! Passive. Each minion the player buys counts (`hero_after_buy`, `counter`, across turns); every
//! 3rd gives them a `Tavern Coin` (if their hand has room).

use crate::cards::{tokens, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 61912;
pub const POWER: CardId = 61917;

/// Minions to buy per Tavern Coin.
const MINIONS: i32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_after_buy(after_buy))]
}

/// After a minion is bought: every 3rd, a Tavern Coin.
fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    if bought.is_spell {
        return;
    }
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter >= MINIONS {
        power.counter -= MINIONS;
        state.add_to_hand(tokens::make_tavern_coin());
    }
}
