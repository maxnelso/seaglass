//! `Great Boar Sticker` (`BG30_MagicItem_988`) — Lesser, 0 Gold, Quilboar.
//!
//! Get 3 Blood Gems. Your Blood Gems give an extra +2/+1.
//!
//! Picked: the Blood Gem bonus (`auras.blood_gem_bonus_atk` / `blood_gem_bonus_hp`, as `Gem
//! Day`'s) goes up by +2/+1 for the rest of the game (the trinket is kept), then 3 Blood Gems go
//! to the hand, or wait for room in a full hand.

use crate::cards::{tokens, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 112013;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.auras.blood_gem_bonus_atk += 2;
    state.auras.blood_gem_bonus_hp += 1;
    for _ in 0..3 {
        state.add_to_hand_or_wait(tokens::make_blood_gem());
    }
}
