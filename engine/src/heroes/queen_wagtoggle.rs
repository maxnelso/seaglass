//! `Queen Wagtoggle` (`TB_BaconShop_HERO_14`) — Armor 14 (Duos 12).
//!
//! Hero Power: `Wax Warband` (`TB_BaconShop_HP_037a`), 0 Gold: Start of Combat: Give a friendly minion of each type +1/+1. (Improves after you spend 10 Gold!)
//!
//! Passive. At Start of Combat a friendly minion of each type (the best assignment of minions to
//! types, as `Misplaced Tea Set`: `docs/tavern.md` §7.8) gains `+N/+N`, `N = 1 + stacks`. Every 10
//! Gold the player spends (`counter`) adds a stack.

use crate::cards::{spells, CardHooks, HeroCombatCtx};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId};
use crate::tavern::TavernState;

pub const ID: HeroId = 57924;
pub const POWER: CardId = 59863;

/// Gold to spend for each improvement.
const GOLD_PER_IMPROVEMENT: i32 = 10;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_start_of_combat(start_of_combat)
            .on_hero_gold_spent(gold_spent),
    )]
}

/// Gold spent counts toward the next improvement.
fn gold_spent(state: &mut TavernState, amount: u32) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.counter += amount as i32;
        while power.counter >= GOLD_PER_IMPROVEMENT {
            power.counter -= GOLD_PER_IMPROVEMENT;
            power.stacks += 1;
        }
    }
}

/// Start of Combat: a friendly minion of each type gains `+N/+N`.
fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let n = 1 + power_mut(ctx.own.auras).map_or(0, |p| p.stacks);
    for i in spells::select_menagerie_targets(ctx.own.board, ctx.own.rng) {
        ctx.own.buff_unit(i, n, n, "Wax Warband");
    }
}
