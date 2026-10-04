//! `Parasitic Fleshling` (`BG36_114`) — Tier 4 Aberration (`4/6`).
//!
//! At the end of your turn, give your left-most minion `+2/+2` (`+4/+4` if Golden).
//! (Improved by each card you've discarded this game!)

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::{CardPool, TavernState};
use crate::rng::Rng;

pub const ID: CardId = 444;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Parasitic Fleshling", 4, 6, 4)
        .with_tribe(Tribe::Aberration)
        .on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, _: &mut Rng) {
    let mult = if state.board[self_idx].is_golden { 2 } else { 1 };
    let buff = (2 + state.auras.cards_discarded as i32) * mult;
    state.board[0].add_stats(buff, buff);
}
