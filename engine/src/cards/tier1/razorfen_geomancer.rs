//! `Razorfen Geomancer` (`BG20_100`) — Tier 1 Quilboar (`2/1`).
//! **Battlecry:** Get `2` (`4` if Golden) **Blood Gems**.

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 116;
pub const NAME: &str = "Razorfen Geomancer";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 1, 1).with_tribe(Tribe::Quilboar)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let count = if unit.is_golden { 4 } else { 2 };
    for _ in 0..count {
        state.add_to_hand(tokens::make_blood_gem());
    }
}
