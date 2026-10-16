//! `Tavish Stormpike` (`BG22_HERO_000`) — Armor 14 (Duos 12).
//!
//! Hero Power: `Lock and Load` (`BG22_HERO_000p_Alt`), 0 Gold: Remove a minion in the Tavern. When you have space next combat, fire it at a random enemy minion.
//!
//! Active, a minion in the Tavern, once per turn: it leaves the shop and the Hero Power keeps it
//! (`stored`, with its stats; its copy out of the pool). Next combat, as soon as the board has
//! room (Start of Combat, step 1b, or `hero_combat_space` once a slot opens), it is summoned at the
//! right end (a summon: the summon observers trigger) and attacks immediately (confirmed). Kept
//! cards are marked once they joined (`Unit::marked`). At the next turn start every kept card's
//! copy goes back to the pool, whether it joined or was lost (confirmed), or as soon as the Hero
//! Power is replaced (`hero_power_removed`: Master Nguyen's next turn start).

use crate::cards::{ActivateTargetKind, BoardCtx, CardHooks};
use crate::combat::MAX_BOARD_SIZE;
use crate::heroes::{power_mut, HERO_SOURCE};
use crate::model::{CardId, HeroId, HeroPowerState};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 77987;
pub const POWER: CardId = 123150;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(lock_and_load)
            .on_hero_power_target(|state, t| !state.shop[t].is_spell)
            .on_hero_start_of_combat(|ctx| join(&mut ctx.own))
            .on_hero_combat_space(join)
            .on_hero_turn_start(back_to_pool)
            .on_hero_power_removed(|_, power, pool| return_kept(power, pool))
            .with_activate_target(ActivateTargetKind::ShopCard),
    )]
}

/// Using it: the Tavern minion is loaded.
fn lock_and_load(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    let Some(t) = target.filter(|&t| t < state.shop.len()) else {
        return;
    };
    let mut minion = state.shop.remove(t);
    minion.frozen = false;
    if let Some(power) = power_mut(&mut state.auras) {
        power.stored.push(minion);
    }
}

/// Room on the board in combat: the loaded minions join at the right end and attack.
fn join(ctx: &mut BoardCtx<'_>) {
    while ctx.board.len() < MAX_BOARD_SIZE {
        let Some(power) = power_mut(ctx.auras) else {
            return;
        };
        let Some(kept) = power.stored.iter_mut().find(|u| !u.marked) else {
            return;
        };
        kept.marked = true;
        let mut minion = kept.clone();
        minion.marked = false;
        ctx.cursor = ctx.board.len();
        let Some(pos) = ctx.summon_as(HERO_SOURCE, minion, "Lock and Load") else {
            return;
        };
        let id = ctx.board[pos].id;
        ctx.pending_attacks.push(id);
    }
}

/// Start of turn: the kept minions' copies go back to the pool.
fn back_to_pool(state: &mut TavernState, pool: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        return_kept(power, pool);
    }
}

/// The kept minions' copies go back to the pool (at the next turn start, or once the Hero Power
/// is replaced: Master Nguyen's next turn).
fn return_kept(power: &mut HeroPowerState, pool: &mut CardPool) {
    for minion in std::mem::take(&mut power.stored) {
        pool.return_unit(&minion);
    }
}
