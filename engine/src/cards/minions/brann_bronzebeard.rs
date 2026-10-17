//! `Brann Bronzebeard` (`BG_LOE_077`) — Tier 5 Neutral (`2/4`).
//!
//! Your Battlecries trigger twice (`three times` if Golden).
//!
//! Battlecries triggered by other cards (`Young Murk-Eye`, `Kelp Keeper`) too. A Battlecry with a
//! target has the same target every time: once it is gone, the repeats do nothing.
//!
//! With `Bronzebeard Portrait` (`trinkets::modifies`, `docs/trinkets.md` §11.8), it is both a
//! Dragon and a Murloc (`Tribe::DragonMurloc`, kept up to date by its `sync_aura`).

use crate::cards::{CardTemplate, Passive};
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 504;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Brann Bronzebeard", 2, 4, 5)
        .with_passive(passive)
        .on_sync_aura(sync_aura)
}

/// Its type: Dragon and Murloc with `Bronzebeard Portrait`; without it, it loses that type.
fn sync_aura(unit: &mut Unit, auras: &PlayerAuras) {
    if crate::trinkets::modifies(auras, ID) {
        unit.tribe = Tribe::DragonMurloc;
    } else if unit.tribe == Tribe::DragonMurloc {
        unit.tribe = Tribe::None;
    }
}

/// Battlecries trigger twice (three times if Golden).
pub fn passive(unit: &Unit, passive: Passive) -> u32 {
    match (passive, unit.is_golden) {
        (Passive::BattlecryTriggers, true) => 3,
        (Passive::BattlecryTriggers, false) => 2,
        _ => 0,
    }
}
