//! `Living Azerite` (`BG28_707`) — Tier 5 Elemental (`8/6`).
//!
//! Whenever you cast a Tavern spell, give Elementals in the Tavern `+4/+3` (`+8/+6` if Golden) this game.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 531;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Living Azerite", 8, 6, 5).with_tribe(Tribe::Elemental)
}

pub fn on_cast_tavern_spell(state: &mut TavernState) {
    let mut mult = 0i32;
    for u in &state.board {
        if u.card_id == ID {
            mult += if u.is_golden { 2 } else { 1 };
        }
    }
    if mult > 0 {
        let d_atk = 4 * mult;
        let d_hp = 3 * mult;
        state.auras.tavern_elemental_atk += d_atk;
        state.auras.tavern_elemental_hp += d_hp;
        for s in &mut state.shop {
            if !s.is_spell && s.tribe.matches(Tribe::Elemental) {
                s.add_stats(d_atk, d_hp);
            }
        }
    }
}
