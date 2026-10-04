//! `Razorfen Flapper` (`BG34_682`) — Tier 4 Quilboar (`5/3`).
//!
//! Battlecry and Deathrattle: Get a (`2` if Golden) Blood Gem Barrage(s).

use crate::cards::{spells, CardTemplate, DeathrattleContext};
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 447;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Razorfen Flapper", 5, 3, 4).with_tribe(Tribe::Quilboar)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        state.add_to_hand(spells::make_blood_gem_barrage());
    }
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let count = if dying.is_golden { 2 } else { 1 };
    for _ in 0..count {
        ctx.add_to_hand(spells::make_blood_gem_barrage());
    }
}
