//! `Silent Deliverer` (`BG36_343`) — Tier 6 Pirate (`7/7`).
//!
//! Battlecry: Get a (`2` if Golden) random Golden minion from Tier 4. It doesn't give a Triple Reward.

use crate::cards::{tier4, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 620;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Silent Deliverer", 7, 7, 6)
        .with_tribe(Tribe::Pirate)
        .on_battlecry(|state, unit, _, _, rng| on_battlecry(state, unit, rng))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, rng: &mut Rng) {
    let catalog = tier4::catalog();
    if catalog.is_empty() {
        return;
    }
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let pick = rng.below(catalog.len());
        let mut golden = catalog[pick].instantiate();
        golden.make_golden();
        state.add_to_hand(golden);
    }
}
