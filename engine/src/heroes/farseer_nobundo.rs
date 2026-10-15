//! `Farseer Nobundo` (`BG31_HERO_003`) — Armor 15 (Duos 13).
//!
//! Hero Power: `The Galaxy's Lens` (`BG31_HERO_003p`), 3 Gold: Get a copy of the last Tavern spell you cast. Each turn, your next Hero Power costs (1) less.
//!
//! Active, untargeted, once per turn: a fresh copy of the last Tavern spell the player cast
//! (`last_tavern_spell_cast`) goes to the hand; it needs one and room in the hand. At each turn
//! start, the first included, it gets 1 cheaper (never below 0; `hero_turn_reset`); using it puts
//! the cost back to 3, as Togwaggle's.

use crate::cards::{spells, CardHooks};
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, HeroState};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 116926;
pub const POWER: CardId = 116924;

/// Its full cost.
const COST: u32 = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(galaxys_lens)
            .on_hero_power_usable(|state| {
                state.auras.last_tavern_spell_cast.is_some() && super::hand_has_room(state)
            })
            .on_hero_turn_reset(discount),
    )]
}

/// Using it: a copy of the last Tavern spell cast; the cost is full again.
fn galaxys_lens(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.cost = COST;
    }
    if let Some(spell) = state
        .auras
        .last_tavern_spell_cast
        .and_then(spells::spell_by_id)
    {
        state.add_to_hand(spell);
    }
}

/// Start of turn: 1 cheaper.
fn discount(hero: &mut HeroState) {
    hero.power.cost = hero.power.cost.saturating_sub(1);
}
