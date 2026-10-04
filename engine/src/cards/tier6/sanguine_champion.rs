//! `Sanguine Champion` (`BG23_017`) — Tier 6 Quilboar (`9/4`).
//!
//! Battlecry and Deathrattle: Your Blood Gems give an extra `+2/+1` (`+4/+2` if Golden) this game.

use crate::cards::{CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 619;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Sanguine Champion", 9, 4, 6).with_tribe(Tribe::Quilboar)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let mult = if unit.is_golden { 2 } else { 1 };
    state.auras.blood_gem_bonus_atk += 2 * mult;
    state.auras.blood_gem_bonus_hp += mult;
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let mult = if dying.is_golden { 2 } else { 1 };
    ctx.auras.blood_gem_bonus_atk += 2 * mult;
    ctx.auras.blood_gem_bonus_hp += mult;
}
