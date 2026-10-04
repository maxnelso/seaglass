//! `Proud Privateer` (`BG33_825`) — Tier 5 Pirate (`8/8`).
//!
//! Your Bounties cast twice (`three times` if Golden).

use crate::cards::{CardTemplate, Passive};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 539;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Proud Privateer", 8, 8, 5)
        .with_tribe(Tribe::Pirate)
        .with_passive(passive)
}

/// Bounty spells are cast twice (three times if Golden).
pub fn passive(unit: &Unit, passive: Passive) -> u32 {
    match (passive, unit.is_golden) {
        (Passive::BountyCasts, true) => 3,
        (Passive::BountyCasts, false) => 2,
        _ => 0,
    }
}
