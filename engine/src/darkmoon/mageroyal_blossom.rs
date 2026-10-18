//! `Mageroyal Blossom` (`BGS_Treasures_101`) — Tier 2 Darkmoon Prize.
//!
//! Discover a Tavern spell of your Tier.
//!
//! A Discover of 3 different Tavern spells of exactly the player's Tier
//! (`spells::draw_discover_tavern_spells_exact_tier`), new cards.

use crate::cards::spells::{self, spell};
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 129193;

/// Options in the Discover.
const OPTIONS: usize = 3;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    let options = spells::draw_discover_tavern_spells_exact_tier(state.tavern_tier, OPTIONS, rng);
    state.push_discover(options);
}
