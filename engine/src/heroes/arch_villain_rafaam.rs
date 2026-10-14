//! `Arch-Villain Rafaam` (`TB_BaconShop_HERO_45`) — Armor 15 (Duos 12).
//!
//! Hero Power: `I'll Take That!` (`TB_BaconShop_HP_053`), 1 Gold: Next combat, get a plain copy of the first minion you kill.
//!
//! Active, untargeted, once per turn: using it arms the next combat (`armed`). There, the first
//! enemy minion a friendly minion kills (`hero_enemy_killed`) gives the player a plain copy of it
//! (`cards::plain_instance`: its card's base version) in their hand, if it has room, using the
//! arm. An arm the combat didn't use is gone at the start of the next turn.

use crate::cards::{self, BoardCtx, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, HeroState, Unit, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 60362;
pub const POWER: CardId = 60381;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(arm)
            .on_hero_turn_reset(disarm)
            .on_hero_enemy_killed(take_that),
    )]
}

/// Using it: the next combat's first kill gives a copy.
fn arm(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.armed = 1;
    }
}

/// Start of turn: the last combat's arm is gone.
fn disarm(hero: &mut HeroState) {
    hero.power.armed = 0;
}

/// The first kill: a plain copy of the victim to the hand.
fn take_that(ctx: &mut BoardCtx<'_>, _: UnitId, victim: &Unit) {
    let Some(power) = power_mut(ctx.auras).filter(|p| p.armed > 0) else {
        return;
    };
    power.armed = 0;
    if let Some(copy) = cards::plain_instance(victim) {
        ctx.add_to_hand(copy);
    }
}
