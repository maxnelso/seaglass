//! `Cap'n Hoggarr` (`BG26_HERO_101`) — Armor 14 (Duos 12).
//!
//! Hero Power: `I'm the Cap'n Now` (`BG26_HERO_101p`), 0 Gold: After you buy a Pirate, gain 1 Gold.
//!
//! Passive. After the player buys a Pirate (a minion with the Pirate type, `hero_after_buy`), they
//! gain 1 Gold.

use crate::cards::CardHooks;
use crate::model::{CardId, HeroId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 101130;
pub const POWER: CardId = 101132;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(POWER, CardHooks::EMPTY.on_hero_after_buy(after_buy))]
}

/// After a Pirate is bought: 1 Gold.
fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    if !bought.is_spell && bought.tribe.matches(Tribe::Pirate) {
        state.gold += 1;
    }
}
