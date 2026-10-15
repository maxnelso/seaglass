//! `Loh, the Living Legend` (`BG33_HERO_001`) — Armor 19 (Duos 17).
//!
//! Hero Power: `Heroic Inspiration` (`BG33_HERO_001p_ALT`), 0 Gold: After 15 friendly minions attack, get a Triple Reward. (15 left!)
//!
//! Passive. Each attack of a friendly minion counts (`hero_friendly_attack`, `counter`, across
//! combats); every 15th gives the player a `Triple Reward` card (`docs/tavern.md` §5.2) in their
//! combat hand, which comes back with it under the usual full-hand rule.

use crate::cards::{tokens, BoardCtx, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, UnitId};

pub const ID: HeroId = 122925;
pub const POWER: CardId = 129164;

/// Friendly attacks per Triple Reward.
const ATTACKS: i32 = 15;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY.on_hero_friendly_attack(friendly_attack),
    )]
}

/// A friendly minion attacked: every 15th attack, a Triple Reward.
fn friendly_attack(ctx: &mut BoardCtx<'_>, _: UnitId) {
    let Some(power) = power_mut(ctx.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter < ATTACKS {
        return;
    }
    power.counter -= ATTACKS;
    let tier = ctx.hero_tier;
    ctx.add_to_hand(tokens::make_triple_reward(tier));
}
