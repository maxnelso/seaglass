//! `Titus Rivendare` (`BG25_354`) — Tier 5 Neutral (`1/7`).
//!
//! Your Deathrattles trigger an extra time (`2` extra times if Golden).

use crate::cards::{CardTemplate, Passive};
use crate::model::{CardId, Unit};

pub const ID: CardId = 551;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Titus Rivendare", 1, 7, 5).with_passive(passive)
}

/// Friendly Deathrattles trigger one extra time (two if Golden) while this is alive.
pub fn passive(unit: &Unit, passive: Passive) -> u32 {
    match passive {
        Passive::ExtraDeathrattles if unit.health > 0 => unit.golden_mult() as u32,
        _ => 0,
    }
}
