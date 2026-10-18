//! `Gorgeous Goblet` (`BGS_Treasures_106`) — Tier 4 Darkmoon Prize.
//!
//! Fill your hand with random Tavern spells.
//!
//! Random Tavern spells the shop can offer, of the player's Tier or lower, one uniform draw each
//! (`super::random_shop_spell`), until the hand holds 10 (`docs/darkmoon_prizes.md` §5: the cards
//! waiting for room take their slots).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 129197;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    for _ in 0..super::hand_room(state) {
        if let Some(card) = super::random_shop_spell(state, |_| true, rng) {
            state.add_to_hand_or_wait(card);
        }
    }
}
