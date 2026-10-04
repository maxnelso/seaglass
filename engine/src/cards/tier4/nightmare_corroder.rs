//! `Nightmare Corroder` (`BG36_115`) — Tier 4 Aberration (`4/5`).
//!
//! At the end of your turn, get a (`2` if Golden) Sludge Corrosion(s).

use crate::cards::{tokens, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::tavern::{CardPool, TavernState};
use crate::rng::Rng;

pub const ID: CardId = 443;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Nightmare Corroder", 4, 5, 4)
        .with_tribe(Tribe::Aberration)
        .on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let count = if state.board[self_idx].is_golden { 2 } else { 1 };
    for _ in 0..count {
        if state.hand.len() >= 10 {
            break;
        }
        state.add_to_hand(tokens::make_sludge_corrosion());
    }
}
