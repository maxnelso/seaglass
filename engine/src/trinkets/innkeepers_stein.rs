//! `Innkeeper's Stein` (`BG30_MagicItem_423`) — Lesser, 0 Gold.
//!
//! The Tavern offers an extra minion of a higher Tier whenever it is Refreshed.
//!
//! `trinket_after_refresh`: every Refresh (the action, a card's, the turn start's new shop) adds a
//! minion of the next Tier (`CardPool::draw_discover_options` of `tavern_tier + 1`, one draw; the
//! highest lower Tier with copies left if none) from the pool at the end of the Tavern, if it has
//! room (7 cards). At Tier 6 there is no higher Tier: nothing (**engine decision**).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState, MAX_SHOP_SIZE};

pub const ID: CardId = 111999;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_refresh(after_refresh))]
}

fn after_refresh(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if state.tavern_tier >= 6 || state.shop.len() >= MAX_SHOP_SIZE {
        return;
    }
    let Some(mut minion) = pool
        .draw_discover_options(state.tavern_tier + 1, 1, rng)
        .into_iter()
        .next()
    else {
        return;
    };
    state.apply_shop_auras(&mut minion);
    state.shop.push(minion);
}
