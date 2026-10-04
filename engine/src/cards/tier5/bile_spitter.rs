//! `Bile Spitter` (`BG33_318`) — Tier 5 Murloc (`1/10`, Venomous).
//!
//! Venomous. Rally: Give another (`2` other if Golden) friendly Murloc Venomous.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 503;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Bile Spitter", 1, 10, 5)
        .with_tribe(Tribe::Murloc)
        .with_keyword(Keyword::Venomous)
}

pub fn on_rally(board: &mut [Unit], attacker_pos: usize, rng: &mut Rng) {
    let count = if board[attacker_pos].is_golden { 2 } else { 1 };
    for _ in 0..count {
        let mut candidates: Vec<usize> = board
            .iter()
            .enumerate()
            .filter(|&(i, u)| {
                i != attacker_pos && u.health > 0 && u.tribe.matches(Tribe::Murloc) && !u.venomous
            })
            .map(|(i, _)| i)
            .collect();
        if candidates.is_empty() {
            candidates = board
                .iter()
                .enumerate()
                .filter(|&(i, u)| {
                    i != attacker_pos && u.health > 0 && u.tribe.matches(Tribe::Murloc)
                })
                .map(|(i, _)| i)
                .collect();
        }
        if candidates.is_empty() {
            break;
        }
        let pick = if candidates.len() == 1 {
            candidates[0]
        } else {
            candidates[rng.below(candidates.len())]
        };
        board[pick].apply_keyword(Keyword::Venomous, false);
    }
}
