//! `Nerubian Deathswarmer` (`BG25_011`) — Tier 2 Undead (`1/4`).
//! **Battlecry:** Your Undead have `+1` (`+2` if Golden) Attack this game *(wherever they are)*.

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, PlayerAuras, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 220;
pub const NAME: &str = "Nerubian Deathswarmer";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 4, 2)
        .with_tribe(Tribe::Undead)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
}

pub fn sync_unit_undead_attack(unit: &mut Unit, auras: &PlayerAuras) {
    if !unit.is_spell && unit.tribe.matches(Tribe::Undead) {
        let diff = auras.undead_bonus_attack - unit.undead_attack_applied;
        if diff != 0 {
            unit.undead_attack_applied = auras.undead_bonus_attack;
            unit.add_stats(diff, 0);
        }
    }
}

pub fn on_battlecry(state: &mut TavernState, unit: &mut Unit) {
    let delta = if unit.is_golden { 2 } else { 1 };
    state.auras.undead_bonus_attack += delta;
    cards::sync_unit_auras(unit, &state.auras);
    state.sync_all_auras();
}
