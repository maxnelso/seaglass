//! `Forsaken Weaver` (`BG34_692`) — Tier 6 Undead (`3/8`).
//!
//! After you cast a Tavern spell, your Undead have `+3` (`+6` if Golden) Attack this game (wherever they are).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 611;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Forsaken Weaver", 3, 8, 6).with_tribe(Tribe::Undead)
}

pub fn after_cast_tavern_spell(state: &mut TavernState) {
    let mut bonus = 0i32;
    for u in &state.board {
        if u.card_id == ID {
            bonus += if u.is_golden { 6 } else { 3 };
        }
    }
    if bonus > 0 {
        state.auras.undead_bonus_attack += bonus;
        state.sync_all_auras();
    }
}
