//! `Heroic Broodmother` (`BG36_849`) — Tier 6 Dragon (`7/7`).
//!
//! Rally: Gain Divine Shield. Start of Combat: Attack immediately (`twice` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 614;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Heroic Broodmother", 7, 7, 6).with_tribe(Tribe::Dragon)
}

pub fn on_rally(attacker: &mut Unit) {
    attacker.apply_keyword(Keyword::DivineShield, false);
}
