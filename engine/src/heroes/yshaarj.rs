//! `Y'Shaarj` (`TB_BaconShop_HERO_92`) — Armor 20 (Duos 18).
//!
//! Hero Power: `Embrace Your Rage` (`TB_BaconShop_HP_103`), 2 Gold: Start of Combat: Summon and get a minion of your Tier.
//!
//! Active, untargeted, once per turn. Using it arms the next combat only (`armed = 1`; using it
//! again before that combat doesn't stack: confirmed). At Start of Combat a random minion of
//! exactly the player's Tier from the game's catalog (`cards::game_templates`) is summoned at the
//! right end of the board if there is room, and a plain copy of it goes to the hand under the
//! usual full-hand rule, whether or not it was summoned (confirmed). A Duos warband that never
//! steps in still gets the card once the combat is over (`hero_sat_out`), with no summon
//! (confirmed). One `rng.below(k)`. That combat uses the arm up either way.

use crate::cards::{self, CardHooks, HeroCombatCtx};
use crate::heroes::{power_mut, HERO_SOURCE};
use crate::model::{CardId, HeroId, PlayerAuras, Unit};
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
            .on_hero_start_of_combat(start_of_combat)
            .on_hero_sat_out(sat_out),
    )]
}

/// Using it: the next combat summons (and gives) a minion.
fn arm(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.armed = 1;
    }
}

/// The armed combat uses the arm up: a random minion of the player's Tier, if there is one.
fn take_minion(auras: &mut PlayerAuras, tier: u32, rng: &mut Rng) -> Option<Unit> {
    let armed = power_mut(auras).map_or(0, |p| std::mem::take(&mut p.armed));
    if armed == 0 {
        return None;
    }
    let candidates: Vec<_> = cards::game_templates(auras)
        .iter()
        .filter(|t| t.tavern_tier == tier)
        .collect();
    let pick = match candidates.len() {
        0 => return None,
        1 => 0,
        k => rng.below(k),
    };
    Some(candidates[pick].instantiate())
}

/// Start of Combat: summon and get a random minion of the player's Tier.
fn start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    let Some(minion) = take_minion(ctx.own.auras, ctx.own.hero_tier, ctx.own.rng) else {
        return;
    };
    ctx.own.cursor = ctx.own.board.len();
    ctx.own.summon_as(HERO_SOURCE, minion.clone(), NAME);
    ctx.own.add_to_hand(minion);
}

/// Duos, the warband never stepped in: the card, with no summon.
fn sat_out(auras: &mut PlayerAuras, tier: u32, rng: &mut Rng) -> Option<Unit> {
    take_minion(auras, tier, rng)
}
