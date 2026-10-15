//! `Guff Runetotem` (`BG20_HERO_242`) — Armor 12 (Duos 10).
//!
//! Hero Power: `Natural Balance` (`BG20_HERO_242p`), 0 Gold: After you buy 20 Tiers' worth of cards, get a Triple Reward. (20 left!)
//!
//! Passive. Each card bought adds its Tier (`hero_after_buy`, `counter`, across turns); every 20
//! give the player a `Triple Reward` card (`docs/tavern.md` §5.2; confirmed), if the hand has
//! room.

use crate::cards::{tokens, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 68129;
pub const POWER: CardId = 68130;

/// Tiers' worth of cards per Triple Reward.
const TIERS: i32 = 20;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_after_buy(after_buy))]
}

/// After a card is bought: its Tier counts; every 20, a Triple Reward.
fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter += bought.tavern_tier as i32;
    let mut rewards = 0;
    while power.counter >= TIERS {
        power.counter -= TIERS;
        rewards += 1;
    }
    for _ in 0..rewards {
        state.add_to_hand(tokens::make_triple_reward());
    }
}
