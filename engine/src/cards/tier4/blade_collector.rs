//! `Blade Collector` (`BG26_817`) — Tier 4 Pirate (`3/2`).
//!
//! Also damages the enemies next to whomever this attacks.

use crate::cards::{CardFlags, CardTemplate};
use crate::model::{CardId, Tribe};

pub const ID: CardId = 405;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Blade Collector", 3, 2, 4)
        .with_tribe(Tribe::Pirate)
        .with_flags(CardFlags::CLEAVE)
}
