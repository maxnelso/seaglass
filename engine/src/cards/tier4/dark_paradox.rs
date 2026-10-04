//! `Dark Paradox` (`BG36_360` / `BG36_360t5`) — Tier 4 All (`2/6`).
//!
//! Rally: Get a (`2` if Golden) random minion(s) of your most common type.

use crate::cards::{self, spells, CardTemplate};
use crate::model::{CardId, PlayerAuras, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 414;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Dark Paradox", 2, 6, 4).with_tribe(Tribe::All)
}

pub fn on_rally(
    board: &[Unit],
    attacker_pos: usize,
    auras: &PlayerAuras,
    generated_hand: &mut Vec<Unit>,
    rng: &mut Rng,
) {
    let count = if board[attacker_pos].is_golden { 2 } else { 1 };
    let tribe = spells::most_common_tribe(board, rng);
    let candidates: Vec<CardTemplate> = cards::full_catalog()
        .into_iter()
        .filter(|t| t.tavern_tier <= 6 && t.card_id != ID && t.tribe.matches(tribe))
        .collect();
    if candidates.is_empty() {
        return;
    }
    for _ in 0..count {
        let idx = rng.below(candidates.len());
        let mut drawn = candidates[idx].instantiate();
        cards::sync_unit_auras(&mut drawn, auras);
        generated_hand.push(drawn);
    }
}
