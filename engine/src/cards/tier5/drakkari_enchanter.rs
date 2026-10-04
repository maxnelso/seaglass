//! `Drakkari Enchanter` (`BG26_ICC_901`) — Tier 5 Neutral (`1/5`).
//!
//! Your end of turn effects trigger twice (`three times` if Golden).

use crate::cards::{CardTemplate, Passive};
use crate::model::{CardId, Unit};

pub const ID: CardId = 512;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Drakkari Enchanter", 1, 5, 5)
        .with_passive(passive)
}

/// End-of-turn effects trigger twice (three times if Golden).
pub fn passive(unit: &Unit, passive: Passive) -> u32 {
    match (passive, unit.is_golden) {
        (Passive::EndOfTurnTriggers, true) => 3,
        (Passive::EndOfTurnTriggers, false) => 2,
        _ => 0,
    }
}
