//! `Great Boar Sticker (Greater)` (`BG30_MagicItem_988t`) — Greater, 0 Gold, Quilboar.
//!
//! Get 5 Blood Gems. Your Blood Gems give an extra +3/+3.
//!
//! As the Lesser one (`great_boar_sticker.rs`): +3/+3, 5 Blood Gems.

use crate::cards::{tokens, CardHooks};
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 115225;

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(ID, CardHooks::EMPTY.on_trinket_bought(bought))]
}

fn bought(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    state.auras.blood_gem_bonus_atk += 3;
    state.auras.blood_gem_bonus_hp += 3;
    for _ in 0..5 {
        state.add_to_hand_or_wait(tokens::make_blood_gem());
    }
}
