//! `Galakrond` (`TB_BaconShop_HERO_02`) — Armor 18 (Duos 16).
//!
//! Hero Power: `Galakrond's Greed` (`TB_BaconShop_HP_011`), 1 Gold: Choose a minion in the Tavern. Then choose a higher Tier minion to replace it.
//!
//! Active, a minion in the Tavern below Tier 6 (a Tier 6 one has no higher Tier: engine
//! decision), once per turn: a Discover among 3 minions of exactly the next Tier from the pool (as
//! a Triple Reward's), kept by the Hero Power (`to_hero_power`). The chosen one replaces the
//! Tavern minion in its slot (`hero_option_chosen`, with the shop's auras), whose copies go back
//! to the pool with the other options'.

use crate::cards::{ActivateTargetKind, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 57635;
pub const POWER: CardId = 57555;

/// The highest Tavern Tier.
const MAX_TIER: u32 = 6;

/// Discover options.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(greed)
            .on_hero_power_target(|state, t| {
                !state.shop[t].is_spell && state.shop[t].tavern_tier < MAX_TIER
            })
            .on_hero_option_chosen(replace)
            .with_activate_target(ActivateTargetKind::ShopCard),
    )]
}

/// Using it: Discover a higher-Tier minion for the Tavern minion's slot.
fn greed(state: &mut TavernState, target: Option<usize>, pool: &mut CardPool, rng: &mut Rng) {
    let Some(t) = target.filter(|&t| t < state.shop.len()) else {
        return;
    };
    let tier = (state.shop[t].tavern_tier + 1).min(MAX_TIER);
    if let Some(power) = power_mut(&mut state.auras) {
        power.counter = t as i32;
    }
    let mut options = pool.draw_discover_options(tier, OPTIONS, rng);
    for option in &mut options {
        option.to_hero_power = true;
    }
    state.push_discover(options);
}

/// The chosen minion replaces the Tavern minion; the rest go back to the pool.
fn replace(state: &mut TavernState, mut chosen: Unit, unchosen: Vec<Unit>, pool: &mut CardPool) {
    for option in &unchosen {
        pool.return_unit(option);
    }
    let slot = state.auras.hero.as_ref().map_or(0, |h| h.power.counter) as usize;
    if state.shop.get(slot).is_some_and(|u| !u.is_spell) {
        pool.return_unit(&state.shop[slot]);
        state.apply_shop_auras(&mut chosen);
        state.shop[slot] = chosen;
    } else {
        pool.return_unit(&chosen);
    }
}
