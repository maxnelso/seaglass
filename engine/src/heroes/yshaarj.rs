//! `Y'Shaarj` (`TB_BaconShop_HERO_92`) — Armor 20 (Duos 18).
//!
//! Hero Power: `Embrace Your Rage` (`TB_BaconShop_HP_103`), 2 Gold: Start of Combat: Summon and get a minion of your Tier.
//!
//! Active, untargeted, once per turn. Using it arms the next combat (`armed += 1`). At Start of
//! Combat, once per arm: a random minion of exactly the player's Tier from the game's catalog
//! (`cards::game_templates`) is summoned at the right end of the board if there is room, and a
//! plain copy of it goes to the hand if it has room (independently: engine decision). One
//! `rng.below(k)` per minion. The arms are used up; a Duos warband that didn't fight keeps them.

use crate::cards::{self, CardHooks, HeroCombatCtx};
use crate::heroes::{power_mut, HERO_SOURCE};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 66196;
pub const POWER: CardId = 66197;

const NAME: &str = "Embrace Your Rage";

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(arm)
            .on_hero_start_of_combat(start_of_combat),
    )]
}

/// Using it: the next combat summons (and gives) a minion.
fn arm(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.armed += 1;
    }
}

/// Start of Combat: for each arm, summon and get a random minion of the player's Tier.
fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let arms = power_mut(ctx.own.auras).map_or(0, |p| std::mem::take(&mut p.armed));
    let tier = ctx.own.hero_tier;
    for _ in 0..arms {
        let candidates: Vec<_> = cards::game_templates(ctx.own.auras)
            .iter()
            .filter(|t| t.tavern_tier == tier)
            .collect();
        let pick = match candidates.len() {
            0 => continue,
            1 => 0,
            k => ctx.own.rng.below(k),
        };
        let minion = candidates[pick].instantiate();
        ctx.own.cursor = ctx.own.board.len();
        ctx.own.summon_as(HERO_SOURCE, minion.clone(), NAME);
        ctx.own.add_to_hand(minion);
    }
}
