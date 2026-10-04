//! `Eredar Escapist` (`BG36_733`) — Tier 6 Demon (`6/8`).
//!
//! After your hero takes 4 damage, get a copy (`two` copies if Golden) of `Corrupted Cupcakes`.

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 609;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Eredar Escapist", 6, 8, 6)
        .with_tribe(Tribe::Demon)
        .on_after_hero_damage(after_hero_damage)
}

pub fn after_hero_damage(state: &mut TavernState, self_idx: usize, amount: i32) {
    if amount <= 0 {
        return;
    }
    let eredar = &mut state.board[self_idx];
    eredar.eredar_damage_progress += amount;
    let mut cupcakes = 0;
    while eredar.eredar_damage_progress >= 4 {
        eredar.eredar_damage_progress -= 4;
        cupcakes += eredar.golden_mult();
    }
    for _ in 0..cupcakes {
        if let Some(card) = spells::spell_by_id(spells::SPELL_CORRUPTED_CUPCAKES) {
            state.add_to_hand(card);
        }
    }
}
