//! `Proud Privateer` (`BG33_825`) — Tier 5 Pirate (`8/8`).
//!
//! Your Bounties cast twice (`three times` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 539;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Proud Privateer", 8, 8, 5).with_tribe(Tribe::Pirate)
}

pub fn bounty_cast_multiplier(board: &[Unit]) -> u32 {
    let mut mult = 1u32;
    for u in board {
        if u.card_id == ID {
            mult = mult.max(if u.is_golden { 3 } else { 2 });
        }
    }
    mult
}
