//! `Nightmare Par-tea Guest` (`BG32_111`) — Tier 5 All (`3/3`).
//!
//! Battlecry and Deathrattle: Get a (`2` if Golden) `Misplaced Tea Set`(s).

use crate::cards::{spells, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 537;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Nightmare Par-tea Guest", 3, 3, 5)
        .with_tribe(Tribe::All)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
        .on_deathrattle(on_deathrattle)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        if let Some(tea) = spells::spell_by_id(spells::SPELL_MISPLACED_TEA_SET) {
            state.add_to_hand(tea);
        }
    }
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        if let Some(tea) = spells::spell_by_id(spells::SPELL_MISPLACED_TEA_SET) {
            ctx.add_to_hand(tea);
        }
    }
}
