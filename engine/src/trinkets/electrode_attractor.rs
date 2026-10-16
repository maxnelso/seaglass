//! `Electrode Attractor` (`BG35_MagicItem_743`) — Greater, 1 Gold, Mech.
//!
//! Magnetic Mechs cost (2). The Tavern offers an extra one whenever it is Refreshed.
//!
//! `trinket_buy_price`: a Magnetic Mech minion costs 2 (`TavernState::buy_price`).
//! `trinket_after_refresh`: every Refresh (the turn start's too) adds a Magnetic Mech of the
//! player's Tier or lower from the pool (`CardPool::draw_discover_filtered`, one draw) at the end
//! of the Tavern, if it has room (7 cards).

use crate::cards::{self, CardHooks};
use crate::model::{CardId, Tribe, TrinketState, Unit};
use crate::rng::Rng;
use crate::tavern::{BuyPrice, CardPool, TavernState, MAX_SHOP_SIZE};

pub const ID: CardId = 131141;

/// The price of a Magnetic Mech.
const PRICE: u32 = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_buy_price(price)
            .on_trinket_after_refresh(after_refresh),
    )]
}

fn magnetic_mech(card_id: CardId) -> bool {
    cards::template(card_id)
        .is_some_and(|t| t.tribe.matches(Tribe::Mech) && t.instantiate().magnetic)
}

fn price(_: &TrinketState, _: &TavernState, card: &Unit, price: BuyPrice) -> BuyPrice {
    if card.is_spell || !magnetic_mech(card.card_id) {
        return price;
    }
    BuyPrice {
        cost: PRICE,
        ..price
    }
}

fn after_refresh(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if state.shop.len() >= MAX_SHOP_SIZE {
        return;
    }
    if let Some(mut mech) = pool
        .draw_discover_filtered(state.tavern_tier, 1, magnetic_mech, rng)
        .pop()
    {
        state.apply_shop_auras(&mut mech);
        state.shop.push(mech);
    }
}
