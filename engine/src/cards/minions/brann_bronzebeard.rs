//! `Brann Bronzebeard` (`BG_LOE_077`) — Tier 5 Neutral (`2/4`).
//!
//! Your Battlecries trigger twice (`three times` if Golden).

use crate::cards::{CardTemplate, Passive};
use crate::model::{CardId, Unit};

pub const ID: CardId = 504;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Brann Bronzebeard", 2, 4, 5).with_passive(passive)
}

/// Battlecries trigger twice (three times if Golden).
pub fn passive(unit: &Unit, passive: Passive) -> u32 {
    match (passive, unit.is_golden) {
        (Passive::BattlecryTriggers, true) => 3,
        (Passive::BattlecryTriggers, false) => 2,
        _ => 0,
    }
}
