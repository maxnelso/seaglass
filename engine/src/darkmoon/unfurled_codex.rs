//! `Unfurled Codex` (`BGS_Treasures_100`) — Tier 1 Darkmoon Prize.
//!
//! Get a random Tavern spell that costs (2) or more.
//!
//! A random Tavern spell the shop can offer, of the player's Tier or lower, whose price is 2 or
//! more (a Health-cost spell by its Health cost), uniform, to the hand or waiting for room
//! (`docs/darkmoon_prizes.md` §5).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 129192;

/// The lowest price of the spell it gives.
const MIN_COST: u32 = 2;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    if let Some(card) = super::random_shop_spell(state, |s| s.spell_cost >= MIN_COST, rng) {
        state.add_to_hand_or_wait(card);
    }
}
