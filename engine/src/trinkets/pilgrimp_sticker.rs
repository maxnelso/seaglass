//! `Pilgrimp Sticker` (`BG32_MagicItem_821`) — Lesser, 3 Gold, Demon.
//!
//! One Demon each turn costs Health instead of Gold to buy. (1 left!)
//!
//! `trinket_buy_price`: the first Demon bought each turn (`armed`, which starts again each
//! turn) costs its price in Health instead of Gold (`TavernState::buy_price`).

use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, TrinketState, Unit};
use crate::rng::Rng;
use crate::tavern::{BuyPrice, CardPool, TavernState};

pub const ID: CardId = 120698;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_turn_start(new_turn)
            .on_trinket_buy_price(price)
            .on_trinket_after_buy(after_buy),
    )]
}

fn eligible(card: &Unit) -> bool {
    !card.is_spell && card.tribe.matches(Tribe::Demon)
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn price(trinket: &TrinketState, _: &TavernState, card: &Unit, price: BuyPrice) -> BuyPrice {
    BuyPrice {
        health: price.health || (trinket.armed == 0 && eligible(card)),
        ..price
    }
}

fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(t) = super::state_mut(&mut state.auras, ID).filter(|_| eligible(bought)) {
        t.armed = 1;
    }
}
