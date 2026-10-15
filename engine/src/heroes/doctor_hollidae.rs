//! `Doctor Holli'dae` (`BG28_HERO_801`) — Armor 14 (Duos 12).
//!
//! Hero Power: `Blessing of the Nine Frogs` (`BG28_HERO_801p`), 1 Gold: Get a random Tavern spell.
//!
//! Active, untargeted, once per turn: a Tavern spell drawn as the shop draws one
//! (`spells::draw_shop_spell`: of the player's Tier or lower, Duos spells too in Duos; engine
//! decision) goes to the hand. It needs room in the hand (engine decision).

use crate::cards::{spells, CardHooks};
use crate::model::{CardId, HeroId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 105434;
pub const POWER: CardId = 110472;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(nine_frogs)
            .on_hero_power_usable(super::hand_has_room),
    )]
}

/// Using it: a random Tavern spell.
fn nine_frogs(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, rng: &mut Rng) {
    let spell = spells::draw_shop_spell(state.tavern_tier, state.auras.duos_game, rng);
    state.add_to_hand(spell);
}
