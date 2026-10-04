//! `Elite Navigator` (`BG32_231`) — Tier 5 Pirate (`5/5`).
//!
//! Battlecry: Make a (`2` if Golden) friendly Pirate(s) from Tier 4 or below Golden.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::TavernState;

pub const ID: CardId = 514;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Elite Navigator", 5, 5, 5).with_tribe(Tribe::Pirate)
}

pub fn on_battlecry(state: &mut TavernState, unit: &Unit, rng: &mut Rng) {
    let count = if unit.is_golden { 2 } else { 1 };
    for _ in 0..count {
        let candidates: Vec<usize> = state
            .board
            .iter()
            .enumerate()
            .filter(|(_, u)| u.tribe.matches(Tribe::Pirate) && u.tavern_tier <= 4 && !u.is_golden)
            .map(|(i, _)| i)
            .collect();
        if candidates.is_empty() {
            break;
        }
        let pick = if candidates.len() == 1 {
            candidates[0]
        } else {
            candidates[rng.below(candidates.len())]
        };
        state.board[pick].make_golden();
    }
    state.sync_all_auras();
}
