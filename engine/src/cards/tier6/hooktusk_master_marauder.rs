//! `Hooktusk, Master Marauder` (`BG36_344`) — Tier 6 Pirate (`4/4`).
//!
//! After you Discover a card, give your other Pirates `+1/+1` (`+2/+2` if Golden).
//! (Improved by Golden minions you played this game!)

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};
use crate::tavern::TavernState;

pub const ID: CardId = 615;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Hooktusk, Master Marauder", 4, 4, 6).with_tribe(Tribe::Pirate)
}

pub fn on_card_discovered(state: &mut TavernState) {
    let base_mult = 1 + state.auras.golden_minions_played as i32;
    let sources: Vec<(usize, i32)> = state
        .board
        .iter()
        .enumerate()
        .filter(|(_, u)| u.card_id == ID)
        .map(|(i, u)| (i, base_mult * (if u.is_golden { 2 } else { 1 })))
        .collect();
    for (src_idx, buff) in sources {
        for (idx, u) in state.board.iter_mut().enumerate() {
            if idx != src_idx && u.tribe.matches(Tribe::Pirate) {
                u.add_stats(buff, buff);
            }
        }
    }
}
