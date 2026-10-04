//! `Red Volumizer` (`BG34_170t`) — Tier 2 Mech (`3/1`).
//! **Magnetic**. The first time this is played or **Magnetized**, your Volumizers have `+3` (`+6` if Golden) Attack this game *(wherever they are)*.

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Keyword, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 223;
pub const NAME: &str = "Red Volumizer";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 1, 2)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::Magnetic)
        .on_play_or_magnetize(on_first_play_or_magnetize)
        .on_aura_bonus(super::blue_volumizer::volumizer_aura_bonus)
        .on_merge_golden(super::blue_volumizer::merge_golden)
}

pub fn on_first_play_or_magnetize(state: &mut TavernState, unit: &mut Unit) {
    if unit.threshold_triggered {
        return;
    }
    unit.threshold_triggered = true;
    let delta_atk = if unit.is_golden { 6 } else { 3 };
    state.auras.volumizer_bonus_atk += delta_atk;
    cards::sync_unit_auras(unit, &state.auras);
    state.sync_all_auras();
}
