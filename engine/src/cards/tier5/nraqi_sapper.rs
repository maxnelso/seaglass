//! `N'raqi Sapper` (`BG36_103`) — Tier 5 Aberration (`6/3`).
//!
//! Battlecry and Deathrattle: Get an (`2` if Golden) `Energizing Chamber`(s).

use crate::cards::{spells, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 536;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "N'raqi Sapper", 6, 3, 5)
        .with_tribe(Tribe::Aberration)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
        .on_deathrattle(on_deathrattle)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        if let Some(chamber) = spells::spell_by_id(spells::SPELL_ENERGIZING_CHAMBER) {
            state.add_to_hand(chamber);
        }
    }
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        if let Some(chamber) = spells::spell_by_id(spells::SPELL_ENERGIZING_CHAMBER) {
            ctx.add_to_hand(chamber);
        }
    }
}
