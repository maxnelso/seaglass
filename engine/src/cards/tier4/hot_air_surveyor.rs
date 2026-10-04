//! `Hot-Air Surveyor` (`BG30_121`) — Tier 4 Quilboar (`3/7`).
//!
//! Blood Gems played from your hand cast an extra time (`2` extra times if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 431;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Hot-Air Surveyor", 3, 7, 4).with_tribe(Tribe::Quilboar)
}

pub fn extra_hand_blood_gem_casts(board: &[Unit]) -> u32 {
    board
        .iter()
        .filter(|u| u.card_id == ID)
        .map(|u| if u.is_golden { 2 } else { 1 })
        .sum()
}
