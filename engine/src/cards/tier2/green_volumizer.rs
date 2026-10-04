//! `Green Volumizer` (`BG34_170t3`) — Tier 2 Mech (`3/3`).
//! **Magnetic**. The first time this is played or **Magnetized**, your Volumizers have `+1/+1` (`+2/+2` if Golden) this game *(wherever they are)*.

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Keyword, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 213;
pub const NAME: &str = "Green Volumizer";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 2)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::Magnetic)
        .on_play_or_magnetize(on_first_play_or_magnetize)
}

pub fn on_first_play_or_magnetize(state: &mut TavernState, unit: &mut Unit) {
    if unit.threshold_triggered {
        return;
    }
    unit.threshold_triggered = true;
    let delta = if unit.is_golden { 2 } else { 1 };
    state.auras.volumizer_bonus_atk += delta;
    state.auras.volumizer_bonus_hp += delta;
    cards::sync_unit_auras(unit, &state.auras);
    state.sync_all_auras();
}
