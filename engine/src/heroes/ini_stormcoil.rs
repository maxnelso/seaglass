//! `Ini Stormcoil` (`BG22_HERO_200`) — Armor 15 (Duos 12).
//!
//! Hero Power: `MechGyver` (`BG22_HERO_200p`), 0 Gold: After 9 friendly minions die, get a random Mech.
//!
//! Passive. Every friendly minion that dies in combat counts (`hero_friendly_death`, `counter`,
//! across combats); every 9th gives the player a random Mech of their Tier or lower from the
//! game's catalog (`cards::game_templates`; engine decision) in their combat hand, which comes
//! back with it under the usual full-hand rule. One `rng.below(k)`.

use crate::cards::{self, BoardCtx, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Tribe, Unit};

pub const ID: HeroId = 80832;
pub const POWER: CardId = 81572;

/// Friendly deaths per Mech.
const DEATHS: i32 = 9;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY.on_hero_friendly_death(friendly_death),
    )]
}

/// A friendly minion died: every 9th, a random Mech.
fn friendly_death(ctx: &mut BoardCtx<'_>, _: &Unit) {
    let Some(power) = power_mut(ctx.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter < DEATHS {
        return;
    }
    power.counter -= DEATHS;
    let tier = ctx.hero_tier;
    let mechs: Vec<_> = cards::game_templates(ctx.auras)
        .iter()
        .filter(|t| t.tavern_tier <= tier && t.tribe.matches(Tribe::Mech))
        .collect();
    let pick = match mechs.len() {
        0 => return,
        1 => 0,
        k => ctx.rng.below(k),
    };
    let mech = mechs[pick].instantiate();
    ctx.add_to_hand(mech);
}
