//! `Bartend-o-Tron's Oilcan` (`BG30_MagicItem_705`) — Lesser, 2 Gold.
//!
//! Reduce the Cost of upgrading the Tavern by (3). At the start of each turn, repeat this.
//!
//! Picked, and at each turn start (`trinket_turn_start`, after the turn's own decay): the Upgrade
//! costs 3 less (`upgrade_cost`, never below 0). At Tier 6 there is nothing to upgrade.

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 117854;

/// The discount.
pub const DISCOUNT: u32 = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(discount)
            .on_trinket_turn_start(discount),
    )]
}

fn discount(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if state.tavern_tier < 6 {
        state.upgrade_cost = state.upgrade_cost.saturating_sub(DISCOUNT);
    }
}
