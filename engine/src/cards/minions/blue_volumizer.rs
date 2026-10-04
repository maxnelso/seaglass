//! `Blue Volumizer` (`BG34_170t2`) — Tier 2 Mech (`1/3`).
//! **Magnetic**. The first time this is played or **Magnetized**, your Volumizers have `+3` (`+6` if Golden) Health this game *(wherever they are)*.

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, Keyword, PlayerAuras, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 202;
pub const NAME: &str = "Blue Volumizer";
/// Card counter shared by all Volumizers: the Attack/Health they have this game.
pub const VOLUMIZER_COUNTER: CardId = ID;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 3, 2)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::Magnetic)
        .on_play_or_magnetize(on_first_play_or_magnetize)
        .on_aura_bonus(volumizer_aura_bonus)
        .on_merge_golden(merge_golden)
}

pub fn on_first_play_or_magnetize(state: &mut TavernState, unit: &mut Unit) {
    if unit.threshold_triggered {
        return;
    }
    unit.threshold_triggered = true;
    let delta_hp = if unit.is_golden { 6 } else { 3 };
    add_volumizer_bonus(&mut state.auras, 0, delta_hp);
    cards::sync_unit_auras(unit, &state.auras);
    state.sync_all_auras();
}

/// The Attack/Health all Volumizers have this game ([`VOLUMIZER_COUNTER`]).
pub fn volumizer_bonus(auras: &PlayerAuras) -> (i32, i32) {
    auras.counter_pair(VOLUMIZER_COUNTER)
}

/// Give all Volumizers `+atk/+hp` this game.
pub fn add_volumizer_bonus(auras: &mut PlayerAuras, atk: i32, hp: i32) {
    auras.add_counter_pair(VOLUMIZER_COUNTER, (atk, hp));
}

/// The shared Volumizer bonus (the `aura_bonus` hook of all three Volumizers).
pub fn volumizer_aura_bonus(_: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    volumizer_bonus(auras)
}

/// Tripled: the Golden's first-play effect can trigger again (the `merge_golden` hook of all
/// three Volumizers).
pub fn merge_golden(_: &[Unit], golden: &mut Unit) {
    golden.threshold_triggered = false;
}
