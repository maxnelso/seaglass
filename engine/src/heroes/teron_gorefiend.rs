//! `Teron Gorefiend` (`BG25_HERO_103`) — Armor 14 (Duos 12).
//!
//! Hero Power: `Rapid Reanimation` (`BG25_HERO_103p`), 0 Gold: Choose a friendly minion. Start of Combat: Destroy it. When you have space, resummon an exact copy.
//!
//! Active, a friendly minion, once per turn: using it marks the minion (`Unit::marked`, until the
//! next combat; confirmed). At Start of Combat the marked minion, if it is on the board, is
//! destroyed (its Deathrattles and Reborn resolve) and an exact copy of it as it was then is kept
//! (`stored`); the copy is summoned at the right end of the board as soon as there is room
//! (`hero_combat_space`: after the destroy's deaths, or after a later round of deaths). A copy
//! that never found room is forgotten at the start of the next turn.

use crate::cards::{ActivateTargetKind, BoardCtx, CardHooks, HeroCombatCtx};
use crate::combat::MAX_BOARD_SIZE;
use crate::heroes::{mark_minion, power_mut, HERO_SOURCE};
use crate::model::{CardId, HeroId, HeroState};

pub const ID: HeroId = 98726;
pub const POWER: CardId = 98728;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(mark_minion)
            .with_activate_target(ActivateTargetKind::BoardAny)
            .on_hero_turn_reset(forget)
            .on_hero_start_of_combat(start_of_combat)
            .on_hero_combat_space(resummon),
    )]
}

/// Start of turn: a copy that never found room is forgotten.
fn forget(hero: &mut HeroState) {
    hero.power.stored.clear();
}

/// Start of Combat: destroy the marked minion and keep an exact copy of it.
fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let Some(unit) = ctx.own.board.iter().find(|u| u.marked && u.health > 0) else {
        return;
    };
    let mut copy = unit.clone();
    copy.marked = false;
    let id = unit.id;
    if let Some(power) = power_mut(ctx.own.auras) {
        power.stored.push(copy);
    }
    ctx.own.destroy(id);
}

/// Room opened: summon the kept copies at the right end, while there is room.
fn resummon(ctx: &mut BoardCtx<'_>) {
    let Some(power) = power_mut(ctx.auras) else {
        return;
    };
    let mut stored = std::mem::take(&mut power.stored);
    while !stored.is_empty() && ctx.board.len() < MAX_BOARD_SIZE {
        let copy = stored.remove(0);
        ctx.cursor = ctx.board.len();
        ctx.summon_as(HERO_SOURCE, copy, "Rapid Reanimation");
    }
    if let Some(power) = power_mut(ctx.auras) {
        power.stored = stored;
    }
}
