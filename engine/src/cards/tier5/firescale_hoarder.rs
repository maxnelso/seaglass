//! `Firescale Hoarder` (`BG32_820`) — Tier 5 Dragon (`5/5`).
//!
//! Battlecry and Deathrattle: Get a (`2` if Golden) `Shiny Ring`(s).

use crate::cards::{spells, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 522;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Firescale Hoarder", 5, 5, 5)
        .with_tribe(Tribe::Dragon)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
        .on_deathrattle(on_deathrattle)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        if let Some(ring) = spells::spell_by_id(spells::SPELL_SHINY_RING) {
            state.add_to_hand(ring);
        }
    }
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        if let Some(ring) = spells::spell_by_id(spells::SPELL_SHINY_RING) {
            ctx.add_to_hand(ring);
        }
    }
}
