//! `Eredar Escapist` (`BG36_733`) — Tier 6 Demon (`6/8`).
//!
//! After your hero takes 4 damage, get a copy (`two` copies if Golden) of `Corrupted Cupcakes`.

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 609;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Eredar Escapist", 6, 8, 6).with_tribe(Tribe::Demon)
}

pub fn on_hero_damage_taken(state: &mut TavernState, amount: i32) {
    if amount <= 0 {
        return;
    }
    let mut cupcakes_to_add = 0u32;
    for u in &mut state.board {
        if u.card_id == ID {
            u.eredar_damage_progress += amount;
            while u.eredar_damage_progress >= 4 {
                u.eredar_damage_progress -= 4;
                cupcakes_to_add += if u.is_golden { 2 } else { 1 };
            }
        }
    }
    for _ in 0..cupcakes_to_add {
        if let Some(cupcakes) = spells::spell_by_id(spells::SPELL_CORRUPTED_CUPCAKES) {
            state.add_to_hand(cupcakes);
        }
    }
}
