//! `Hand of Fate` (`TB_BaconShop_HERO_35_Buddy_t2`) — Wheel of Yogg-Saron, 19%.
//!
//! Get 2 random Darkmoon Prizes.
//!
//! 2 prizes (confirmed), each one `rng.below(k)` among every implemented prize of any Tier, the
//! minion prize included (`darkmoon::random_prize`; two independent draws: engine decision), new
//! prize cards that wait for room in a full hand (`TavernState::add_to_hand_or_wait`, confirmed;
//! `docs/wheel_of_yogg.md` §4).

use crate::cards::spells::spell;
use crate::cards::CardHooks;
use crate::darkmoon;
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 80434;

/// The prizes it gives (confirmed).
const PRIZES: usize = 2;

/// The outcome's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, spell(cast))]
}

fn cast(state: &mut TavernState, _: &Unit, _: usize, _: &mut CardPool, rng: &mut Rng) {
    for _ in 0..PRIZES {
        if let Some(prize) = darkmoon::random_prize(state, rng) {
            state.add_to_hand_or_wait(prize);
        }
    }
}
