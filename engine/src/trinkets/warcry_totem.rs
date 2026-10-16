//! `Warcry Totem` (`BG36_MagicItem_202`) — Lesser, 4 Gold, Dragon, Murloc.
//!
//! The first two Battlecry minions you buy each turn are free. (2 left!)
//!
//! `trinket_buy_price`: the first 2 Battlecry minions bought each turn (`armed`, which starts again
//! each turn) cost nothing (`TavernState::buy_price`).

use crate::cards::{self, CardHooks};
use crate::model::{CardId, TrinketState, Unit};
use crate::rng::Rng;
use crate::tavern::{BuyPrice, CardPool, TavernState};

pub const ID: CardId = 133389;

/// Free Battlecry minions per turn.
const PER_TURN: u32 = 2;

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
    !card.is_spell && cards::is_battlecry_minion(card.card_id)
}

fn new_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    super::reset_per_turn(state, ID);
}

fn price(trinket: &TrinketState, _: &TavernState, card: &Unit, price: BuyPrice) -> BuyPrice {
    if trinket.armed < PER_TURN && eligible(card) {
        BuyPrice { cost: 0, ..price }
    } else {
        price
    }
}

fn after_buy(state: &mut TavernState, bought: &Unit, _: &mut CardPool, _: &mut Rng) {
    if let Some(t) = super::state_mut(&mut state.auras, ID).filter(|_| eligible(bought)) {
        t.armed = (t.armed + 1).min(PER_TURN);
    }
}
