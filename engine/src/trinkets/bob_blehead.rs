//! `Bob-blehead` (`BG30_MagicItem_998`) — Lesser, 0 Gold.
//!
//! Gain 2 Gold. The Tavern no longer offers cards from Tier 1 or Tier 2.
//!
//! 2 Gold as picked. `trinket_shop_tiers`: the cards every shop draw offers (any Refresh, the turn
//! start's too; minions and the spell) are of Tier 3 or higher; below Tier 3, of Tier 3 **(engine
//! decision)**. The shop as it is when picked stays.

use crate::cards::CardHooks;
use crate::model::{CardId, TrinketState};
use crate::rng::Rng;
use crate::tavern::{CardPool, RefreshKind, TavernState};

pub const ID: CardId = 113101;

/// The lowest Tier the Tavern offers.
const MIN_TIER: u32 = 3;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(bought)
            .on_trinket_shop_tiers(tiers),
    )]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.gold += 2;
}

fn tiers(_: &TrinketState, _: RefreshKind, (min, max): (u32, u32)) -> (u32, u32) {
    let min = min.max(MIN_TIER);
    (min, max.max(min))
}
