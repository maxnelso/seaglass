//! `Ominous Stone` (`BG36_MagicItem_206`) — Lesser, 3 Gold.
//!
//! Discover a Tier 4 minion of your most common type with a Dark Gift.
//!
//! As picked: a Dark Gift offer (`docs/dark_gifts.md` §12.3) of Tier 4 minions from the pool, all
//! of the player's most common type as far as the pool has them, each with a different gift by
//! the turn's rules (engine decision). Offered only in games with Dark Gifts
//! (`CardFlags::DARK_GIFTS`).

use crate::cards::{CardFlags, CardHooks};
use crate::dark_gifts::{self, GiftPool, OfferRules, TypeRule};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 133393;

/// The Tier offered.
const TIER: u32 = 4;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .with_flags(CardFlags::DARK_GIFTS)
            .on_trinket_bought(discover),
    )]
}

fn discover(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let rules = OfferRules {
        turn: state.turn,
        tiers: vec![TIER],
        most_common: TypeRule::All,
        gifts: GiftPool::Turn,
        golden: false,
    };
    dark_gifts::discover(state, pool, rng, &rules);
}
