//! `Stuffed Coin Purse` (`BG35_MagicItem_814`) — Lesser, 0 Gold.
//!
//! When you reach Tier 6, gain 12 gold.
//!
//! `trinket_after_upgrade`: the Upgrade to Tier 6 gives 12 Gold, once (`stacks`). Picked at Tier 6
//! already, it gives them at once (**engine decision**).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 130832;

/// The Tier to reach.
const TIER: u32 = 6;
/// The Gold it gives.
const GOLD: u32 = 12;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(check)
            .on_trinket_after_upgrade(check),
    )]
}

fn check(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    if state.tavern_tier < TIER {
        return;
    }
    if let Some(purse) = super::state_mut(&mut state.auras, ID).filter(|t| t.stacks == 0) {
        purse.stacks = 1;
        state.gold += GOLD;
    }
}
