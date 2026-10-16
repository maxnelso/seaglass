//! `Frigid Blossom` (`BG36_MagicItem_300`) — Lesser, 0 Gold, Elemental.
//!
//! After you Refresh, reduce the cost of upgrading the Tavern by (1).
//!
//! `trinket_after_player_refresh`: every Refresh the player does (the action, or one of their
//! cards' or Hero Powers'; not the turn start's new shop: confirmed, `docs/tavern.md` §4.1)
//! lowers `upgrade_cost` by 1, down to 0.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133340;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY.on_trinket_after_player_refresh(after_refresh),
    )]
}

fn after_refresh(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.upgrade_cost = state.upgrade_cost.saturating_sub(1);
}
