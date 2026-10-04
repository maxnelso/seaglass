//! `Auto Accelerator` (`BG34_170`) — Tier 3 Mech (`3/3`).
//! **Battlecry:** Get a (`2` if Golden) random **Magnetic** Volumizer(s).

use crate::cards::{minions, CardTemplate};
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 305;
pub const NAME: &str = "Auto Accelerator";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 3)
        .with_tribe(Tribe::Mech)
        .on_battlecry(|state, unit, _, _, rng| on_battlecry(state, unit, rng))
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, rng: &mut Rng) {
    let volumizers = [
        minions::blue_volumizer::template(),
        minions::green_volumizer::template(),
        minions::red_volumizer::template(),
    ];
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        if state.hand.len() < 10 {
            let pick = rng.below(volumizers.len());
            let mut vol = volumizers[pick].instantiate();
            state.apply_global_unit_auras(&mut vol);
            state.add_to_hand(vol);
        }
    }
}
