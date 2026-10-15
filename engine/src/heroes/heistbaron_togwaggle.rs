//! `Heistbaron Togwaggle` (`BG23_HERO_305`) — Armor 16 (Duos 14).
//!
//! Hero Power: `The Perfect Crime` (`BG23_HERO_305p`), 11 Gold: Steal all cards from the Tavern. Each turn, your next Hero Power costs (1) less.
//!
//! Active, untargeted, once per turn: every shop card goes to the hand while it has room, left to
//! right (not a buy); the rest stay. At the start of each turn, the first included, it gets 1
//! cheaper (never below 0; `hero_turn_reset`; engine decision); using it puts the cost back to
//! 11. It needs a card in the shop and room in the hand.

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, HeroState};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 86291;
pub const POWER: CardId = 86292;

/// Its full cost.
const COST: u32 = 11;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_power(perfect_crime)
            .on_hero_power_usable(|state| !state.shop.is_empty() && super::hand_has_room(state))
            .on_hero_turn_reset(discount),
    )]
}

/// Using it: the shop's cards to the hand; the cost is full again.
fn perfect_crime(state: &mut TavernState, _: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    if let Some(power) = power_mut(&mut state.auras) {
        power.cost = COST;
    }
    while !state.shop.is_empty() && super::hand_has_room(state) {
        let card = state.shop.remove(0);
        state.add_to_hand(card);
    }
}

/// Start of turn: 1 cheaper.
fn discount(hero: &mut HeroState) {
    hero.power.cost = hero.power.cost.saturating_sub(1);
}
