//! `Mysterious K'Thir` (`BG36_320`) — Tier 5 Aberration (`7/7`).
//!
//! At the end of your turn, discard your 3 (`6` if Golden) left-most Tavern spells. Gain `+7/+7` for each discarded.

use crate::cards::{spells, CardTemplate};
use crate::model::{CardId, Tribe};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 534;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Mysterious K'Thir", 7, 7, 5)
        .with_tribe(Tribe::Aberration)
        .on_end_of_turn(on_end_turn)
}

pub fn on_end_turn(state: &mut TavernState, self_idx: usize, pool: &mut CardPool, rng: &mut Rng) {
    let max_discards = if state.board[self_idx].is_golden { 6 } else { 3 };
    let mut discarded_count = 0i32;
    for _ in 0..max_discards {
        let Some(h_idx) = state
            .hand
            .iter()
            .position(|h| h.is_spell && spells::is_tavern_spell(h.card_id))
        else {
            break;
        };
        state.discard_hand_card(h_idx, pool, rng);
        discarded_count += 1;
    }
    if discarded_count > 0 && self_idx < state.board.len() {
        state.board[self_idx].add_stats(7 * discarded_count, 7 * discarded_count);
    }
}
