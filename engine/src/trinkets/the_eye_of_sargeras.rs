//! `The Eye of Sargeras` (`BG30_MagicItem_701`) — Greater, 5 Gold, Demon.
//!
//! Every fourth card you buy costs Health instead of Gold. (3 left!)
//!
//! `trinket_buy_price`: the 4th card bought after it is picked, then every 4th (`counter`, the
//! cards bought since, across turns), costs its price in Health instead of Gold
//! (`TavernState::buy_price`). `trinket_after_buy` counts the buys.

use crate::cards::CardHooks;
use crate::model::{CardId, TrinketState, Unit};
use crate::rng::Rng;
use crate::tavern::{BuyPrice, CardPool, TavernState};

pub const ID: CardId = 117795;

/// Buys per Health buy.
const BUYS: i32 = 4;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_buy_price(price)
            .on_trinket_after_buy(after_buy),
    )]
}

fn price(trinket: &TrinketState, _: &TavernState, _: &Unit, price: BuyPrice) -> BuyPrice {
    BuyPrice {
        health: price.health || trinket.counter == BUYS - 1,
        ..price
    }
}

fn after_buy(state: &mut TavernState, _: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(t) = super::state_mut(&mut state.auras, ID) {
        t.counter = (t.counter + 1) % BUYS;
    }
}
