//! `Lord Jaraxxus` (`TB_BaconShop_HERO_37`) — Armor 15 (Duos 12).
//!
//! Hero Power: `Bloodfury` (`TB_BaconShop_HP_036`), 0 Gold: After friendly minions deal 150 damage, open a portal to the Twisting Nether! (150 left!)
//!
//! Passive (`docs/heroes.md` §11.2): `hero_friendly_damage_dealt` counts damage dealt by friendly
//! minions (`counter`) across combats and Tavern triggers; once it reaches 150, the Hero Power
//! transforms into `Nether Portal` (`TB_BaconShop_HP_036t`, 0 Gold passive: at the start of each
//! turn, get 2 random Demons of the player's Tier or lower from the pool).

use crate::cards::{BoardCtx, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Tribe, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 59807;
pub const POWER: CardId = 59808;

/// `Nether Portal` (`TB_BaconShop_HP_036t`).
pub const NETHER_PORTAL: CardId = 123087;

const DAMAGE_REQUIRED: i32 = 150;
const DEMONS_PER_TURN: usize = 2;

pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            POWER,
            CardHooks::EMPTY.on_hero_friendly_damage_dealt(friendly_damage_dealt),
        ),
        (
            NETHER_PORTAL,
            CardHooks::EMPTY.on_hero_turn_start(nether_portal),
        ),
    ]
}

fn friendly_damage_dealt(ctx: &mut BoardCtx<'_>, _: UnitId, amount: i32) {
    if amount <= 0 {
        return;
    }
    let Some(power) = power_mut(ctx.auras) else {
        return;
    };
    power.counter += amount;
    if power.counter >= DAMAGE_REQUIRED {
        power.card_id = NETHER_PORTAL;
        power.cost = 0;
        power.uses_left = 0;
        power.counter = 0;
    }
}

fn nether_portal(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    for _ in 0..DEMONS_PER_TURN {
        if let Some(demon) = pool.draw_by_tribe(Tribe::Demon, None, state.tavern_tier, rng) {
            state.add_pool_card_to_hand(demon, pool);
        }
    }
}
