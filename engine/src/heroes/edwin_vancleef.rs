//! `Edwin VanCleef` (`TB_BaconShop_HERO_01`) — Armor 15 (Duos 15).
//!
//! Hero Power: `Sharpen Blades` (`TB_BaconShop_HP_001`), 1 Gold: Give a minion +2/+2. Improves after you buy 4 cards. (4 left!)
//!
//! Active, a friendly minion, once per turn: it gains `+(2 + stacks)/+(2 + stacks)`. Every 4th
//! card bought (minions and spells, counted across turns: `hero_after_buy`, `counter`) adds a
//! stack: each improvement is +1/+1 (engine decision).

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 57633;
pub const POWER: CardId = 57567;

/// Cards to buy per improvement.
const BUYS: i32 = 4;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(sharpen_blades)
            .on_hero_after_buy(after_buy)
            .with_activate_target(ActivateTargetKind::BoardAny),
    )]
}

/// Using it: `+(2 + stacks)/+(2 + stacks)`.
fn sharpen_blades(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    let bonus = 2 + power_mut(&mut state.auras).map_or(0, |p| p.stacks);
    if let Some(unit) = target.and_then(|t| state.board.get_mut(t)) {
        unit.add_stats(bonus, bonus);
    }
}

/// After a card is bought: every 4th improves it.
fn after_buy(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.counter += 1;
        if power.counter >= BUYS {
            power.counter -= BUYS;
            power.stacks += 1;
        }
    }
}
