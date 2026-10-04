//! `Leyline Surfacer` (`BG35_881`) — Tier 4 Elemental (`4/6`).
//!
//! Battlecry and Deathrattle: Get an (`2` if Golden) Arcane Absorption(s).

use crate::cards::{tokens, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 437;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Leyline Surfacer", 4, 6, 4).with_tribe(Tribe::Elemental)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        state.add_to_hand(tokens::make_arcane_absorption());
    }
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        ctx.add_to_hand(tokens::make_arcane_absorption());
    }
}
