//! `Flighty Scout` (`BG32_330`) — Tier 1 Murloc (`3/3`).
//! **Start of Combat:** If this minion is in your hand, summon a copy of it.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};

pub const ID: CardId = 113;
pub const NAME: &str = "Flighty Scout";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 1)
        .with_tribe(Tribe::Murloc)
        .on_combat_copies_from_hand(|card| card.golden_mult() as u32)
}
