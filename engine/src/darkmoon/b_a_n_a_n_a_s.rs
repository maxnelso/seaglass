//! `B.A.N.A.N.A.S.` (`BGS_Treasures_019`) — Tier 3 Darkmoon Prize.
//!
//! Fill your hand with Tavern Dish Bananas.
//!
//! `Tavern Dish Banana`s until the hand holds 10 (`docs/darkmoon_prizes.md` §5: the cards waiting
//! for room take their slots).

use crate::cards::spells::{self, spell};
use crate::cards::CardHooks;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 65264;

/// The prize's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, _: &mut Rng) {
    for _ in 0..super::hand_room(state) {
        state.add_to_hand_or_wait(spells::make_tavern_dish_banana());
    }
}
