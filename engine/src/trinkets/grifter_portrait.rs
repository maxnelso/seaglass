//! `Grifter Portrait` (`BG32_MagicItem_957`) — Lesser, 2 Gold, Pirate.
//!
//! Get a Doubloon Grifter. The first Pirate you buy each turn is free.
//!
//! Picked: a plain `Doubloon Grifter`, a minion in no pool (`docs/trinkets.md` §11.9).
//! `trinket_buy_price`: the first Pirate minion bought each turn (`armed`, which starts again each
//! turn; the turn it is picked too) costs nothing (`TavernState::buy_price`).

use crate::cards::minions::doubloon_grifter;
use crate::cards::CardHooks;
use crate::model::{CardId, Tribe, TrinketState, Unit};
use crate::rng::Rng;
use crate::tavern::{BuyPrice, CardPool, TavernState};

pub const ID: CardId = 121036;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_turn_start(new_turn)
            .on_trinket_buy_price(price)
            .on_trinket_after_buy(after_buy),
    )]
}

fn eligible(card: &Unit) -> bool {
    !card.is_spell && card.tribe.matches(Tribe::Pirate)
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::give_minion(state, doubloon_grifter::ID);
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn price(trinket: &TrinketState, _: &TavernState, card: &Unit, price: BuyPrice) -> BuyPrice {
    if trinket.armed == 0 && eligible(card) {
        BuyPrice { cost: 0, ..price }
    } else {
        price
    }
}

fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(t) = super::state_mut(&mut state.auras, ID).filter(|_| eligible(bought)) {
        t.armed = 1;
    }
}
