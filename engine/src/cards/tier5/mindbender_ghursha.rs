//! `Mindbender Ghur'sha` (`BG36_097`) — Tier 5 Aberration (`3/9`).
//!
//! Whenever you discard a card, give your other minions `+4/+4` (`+8/+8` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 533;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Mindbender Ghur'sha", 3, 9, 5).with_tribe(Tribe::Aberration)
}

pub fn on_discard(state: &mut TavernState) {
    let sources: Vec<(usize, i32)> = state
        .board
        .iter()
        .enumerate()
        .filter(|(_, u)| u.card_id == ID)
        .map(|(i, u)| (i, if u.is_golden { 8 } else { 4 }))
        .collect();
    for (src_idx, buff) in sources {
        for (idx, u) in state.board.iter_mut().enumerate() {
            if idx != src_idx {
                u.add_stats(buff, buff);
            }
        }
    }
}
