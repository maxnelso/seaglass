//! `Headhunter Gryphon` (`BG36_204`) — Tier 4 Beast (`3/5`).
//!
//! Rally: Get a (`2` if Golden) random Beast(s).

use crate::cards::{self, CardTemplate};
use crate::model::{CardId, PlayerAuras, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 427;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Headhunter Gryphon", 3, 5, 4).with_tribe(Tribe::Beast)
}

pub fn on_rally(
    attacker: &Unit,
    auras: &PlayerAuras,
    generated_hand: &mut Vec<Unit>,
    rng: &mut Rng,
) {
    let count = if attacker.is_golden { 2 } else { 1 };
    let candidates: Vec<CardTemplate> = cards::full_catalog()
        .into_iter()
        .filter(|t| t.card_id != ID && t.tribe.matches(Tribe::Beast))
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
