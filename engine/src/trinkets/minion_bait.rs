//! `Minion Bait` (`BG30_MagicItem_973`) — Lesser, 0 Gold.
//!
//! The Tavern offers 2 extra 92 whenever it is Refreshed.
//!
//! The `92` is the minion type the trinket names, assigned when it is offered
//! (`docs/trinkets.md` §3.4, confirmed). `trinket_after_refresh`: every Refresh (the turn start's
//! too, as `Innkeeper's Stein`'s) adds 2 minions of that type of the player's Tier or lower from
//! the pool (`CardPool::draw_by_tribe`, one draw each) at the end of the Tavern, while it has
//! room.

use crate::cards::{CardFlags, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState, MAX_SHOP_SIZE};

pub const ID: CardId = 111962;

/// The extra minions per Refresh.
const EXTRA: usize = 2;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::TRINKET_MINION_TYPE)
            .on_trinket_after_refresh(after_refresh),
    )]
}

fn after_refresh(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let Some(tribe) = super::named_type(&state.auras, ID) else {
        return;
    };
    for _ in 0..EXTRA {
        if state.shop.len() >= MAX_SHOP_SIZE {
            return;
        }
        let Some(mut minion) = pool.draw_by_tribe(tribe, None, state.tavern_tier, rng) else {
            return;
        };
        state.apply_shop_auras(&mut minion);
        state.shop.push(minion);
    }
}
