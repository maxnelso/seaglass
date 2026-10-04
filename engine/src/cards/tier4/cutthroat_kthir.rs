//! `Cutthroat K'Thir` (`BG36_106`) — Tier 4 Aberration (`4/4`).
//!
//! Whenever you discard a card, give this and your Deity `+4/+4` (`+8/+8` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 413;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Cutthroat K'Thir", 4, 4, 4).with_tribe(Tribe::Aberration)
}

pub fn on_discard(state: &mut TavernState) {
    let mut deity_buff = 0i32;
    for u in &mut state.board {
        if u.card_id == ID {
            let buff = if u.is_golden { 8 } else { 4 };
            u.add_stats(buff, buff);
            deity_buff += buff;
        }
    }
    if deity_buff > 0 {
        state.auras.deity.attack += deity_buff;
        state.auras.deity.health += deity_buff;
    }
}
