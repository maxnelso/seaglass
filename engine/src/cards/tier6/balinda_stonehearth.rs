//! `Balinda Stonehearth` (`BG35_883`) — Tier 6 Neutral (`6/6`).
//!
//! Your spells that target friendly minions cast twice (`three times` if Golden).

use crate::cards::{CardTemplate, Passive};
use crate::model::{CardId, Unit};

pub const ID: CardId = 602;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Balinda Stonehearth", 6, 6, 6).with_passive(passive)
}

/// Targeted Tavern spells are cast twice (three times if Golden).
pub fn passive(unit: &Unit, passive: Passive) -> u32 {
    match (passive, unit.is_golden) {
        (Passive::TargetedSpellCasts, true) => 3,
        (Passive::TargetedSpellCasts, false) => 2,
        _ => 0,
    }
}
