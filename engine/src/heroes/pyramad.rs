//! `Pyramad` (`TB_BaconShop_HERO_39`) — Armor 14 (Duos 12).
//!
//! Hero Power: `Brick by Brick` (`TB_BaconShop_HP_040`), 2 Gold: Steal a random minion from the Tavern. Double its Health.
//!
//! Active, untargeted, once per turn: a random minion in the shop (one `rng.below(k)`) gets twice
//! its Health and goes to the hand (not a buy). It needs a minion in the shop and room in the
//! hand.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 59831;
pub const POWER: CardId = 59832;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(brick_by_brick)
            .on_hero_power_usable(|state| {
                super::hand_has_room(state) && state.shop.iter().any(|u| !u.is_spell)
            }),
    )]
}

/// Using it: a random shop minion, Health doubled, to the hand.
fn brick_by_brick(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, rng: &mut Rng) {
    let minions: Vec<usize> = (0..state.shop.len())
        .filter(|&i| !state.shop[i].is_spell)
        .collect();
    let pick = match minions.len() {
        0 => return,
        1 => minions[0],
        k => minions[rng.below(k)],
    };
    let mut minion = state.shop.remove(pick);
    let health = minion.health.max(0);
    minion.add_stats(0, health);
    state.add_to_hand(minion);
}
