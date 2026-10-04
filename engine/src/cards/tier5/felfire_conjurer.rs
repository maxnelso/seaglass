//! `Felfire Conjurer` (`BG32_821`) — Tier 5 Demon / Dragon (`6/5`).
//!
//! At the end of your turn, your Tavern spells give an extra `+1/+1` (`+2/+2` if Golden) this game.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::{CardPool, TavernState};
use crate::rng::Rng;

pub const ID: CardId = 520;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Felfire Conjurer", 6, 5, 5)
        .with_tribe(Tribe::DemonDragon)
        .on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let bonus = if state.board[self_idx].is_golden { 2 } else { 1 };
    state.auras.spell_bonus_atk += bonus;
    state.auras.spell_bonus_hp += bonus;
}
