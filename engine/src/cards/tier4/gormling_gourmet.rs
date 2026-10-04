//! `Gormling Gourmet` (`BG32_336`) — Tier 4 Murloc (`4/3`, Taunt).
//!
//! Taunt. Battlecry and Deathrattle: Get a (`2` if Golden) Seafood Stew(s).

use crate::cards::{spells, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Keyword, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 425;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Gormling Gourmet", 4, 3, 4)
        .with_tribe(Tribe::Murloc)
        .with_keyword(Keyword::Taunt)
        .on_battlecry(|state, unit, _, _, _| on_battlecry(state, unit))
        .on_deathrattle(on_deathrattle)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        state.add_to_hand(spells::make_seafood_stew());
    }
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        ctx.add_to_hand(spells::make_seafood_stew());
    }
}
