//! `Charging Czarina` (`BG28_741`) — Tier 5 Mech (`4/1`, Divine Shield).
//!
//! Divine Shield. Whenever you cast a Tavern spell, give your minions with Divine Shield `+4` (`+8` if Golden) Attack.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 506;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Charging Czarina", 4, 1, 5)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::DivineShield)
}

pub fn on_cast_tavern_spell(state: &mut TavernState) {
    let mut total_atk = 0i32;
    for u in &state.board {
        if u.card_id == ID {
            total_atk += if u.is_golden { 8 } else { 4 };
        }
    }
    if total_atk > 0 {
        for u in &mut state.board {
            if u.divine_shield {
                u.add_stats(total_atk, 0);
            }
        }
    }
}
