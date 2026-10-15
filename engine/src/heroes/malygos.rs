//! `Malygos` (`TB_BaconShop_HERO_58`) — Armor 17 (Duos 15).
//!
//! Hero Power: `Arcane Alteration` (`TB_BaconShop_HP_052`), 0 Gold: Replace a card with a random one of the same Tier. (Twice per turn.)
//!
//! Active, a card in the Tavern (engine decision: the Tavern's cards), twice per turn (the first
//! use of a turn gives the use back, as Blackthorn's): the card is replaced in its slot, a minion
//! by a random minion of exactly its Tier from the pool (its copies return first), a spell by a
//! random Tavern spell of exactly its Tier.

use crate::cards::{spells, ActivateTargetKind, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, HeroState};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 61490;
pub const POWER: CardId = 60378;

/// Uses per turn.
const USES: i32 = 2;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(arcane_alteration)
            .on_hero_turn_reset(new_turn)
            .with_activate_target(ActivateTargetKind::ShopCard),
    )]
}

/// Using it: replace the Tavern card; the turn's second use.
fn arcane_alteration(
    state: &mut TavernState,
    target: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.counter += 1;
        if power.counter < USES {
            power.uses_left += 1;
        }
    }
    let Some(t) = target.filter(|&t| t < state.shop.len()) else {
        return;
    };
    let tier = state.shop[t].tavern_tier;
    let new = if state.shop[t].is_spell {
        spells::draw_discover_tavern_spells_exact_tier(tier, 1, rng).pop()
    } else {
        pool.return_unit(&state.shop[t]);
        pool.draw_discover_options(tier, 1, rng)
            .pop()
            .map(|mut minion| {
                state.apply_shop_auras(&mut minion);
                minion
            })
    };
    if let Some(new) = new {
        state.shop[t] = new;
    }
}

/// Start of turn: no use counted yet.
fn new_turn(hero: &mut HeroState) {
    hero.power.counter = 0;
}
