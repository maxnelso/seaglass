//! `Hot-Air Surveyor` (`BG30_121`) — Tier 4 Quilboar (`3/7`).
//!
//! Blood Gems played from your hand cast an extra time (`2` extra times if Golden).
//!
//! Each extra cast is a cast of its own (`docs/tavern.md` §7.4): `Felboar` counts it, and the
//! "whenever you cast a spell on" minions (`Glambot`, ...) and `Geomagus Roogug` trigger on it.
//! `Balinda Stonehearth`'s extra casts add to these (1 + 1 + 1 = 3 casts). Blood Gems a card casts
//! or plays (`Victorious Geomant`, `Geomagus Roogug`) aren't played from your hand: no extra cast.

use crate::cards::{CardTemplate, Passive};
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 431;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Hot-Air Surveyor", 3, 7, 4)
        .with_tribe(Tribe::Quilboar)
        .with_passive(passive)
}

/// Blood Gems played from hand are cast one extra time (two if Golden).
pub fn passive(unit: &Unit, passive: Passive) -> u32 {
    match passive {
        Passive::ExtraHandBloodGemCasts => unit.golden_mult() as u32,
        _ => 0,
    }
}
