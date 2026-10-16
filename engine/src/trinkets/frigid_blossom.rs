//! `Frigid Blossom` (`BG36_MagicItem_300`) — Lesser, 0 Gold, Elemental.
//!
//! After you Refresh, reduce the cost of upgrading the Tavern by (1).
//!
//! `trinket_after_refresh`: every Refresh (the action, a card's, the turn start's new shop, as for
//! `hero_after_refresh`: **engine decision**) lowers `upgrade_cost` by 1, down to 0.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133340;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_after_refresh(after_refresh))]
}

fn after_refresh(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.upgrade_cost = state.upgrade_cost.saturating_sub(1);
}
