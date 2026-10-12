//! `Flighty Scout` (`BG32_330`) — Tier 1 Murloc (`3/3`).
//! **Start of Combat:** If this minion is in your hand, summon a copy of it.
//!
//! The copy doesn't lock the card in hand (`docs/combat.md` §2): `Diremuck Forager` or `Expert
//! Aviator` can still summon it from hand this combat.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};

pub const ID: CardId = 113;
pub const NAME: &str = "Flighty Scout";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 1)
        .with_tribe(Tribe::Murloc)
        .on_combat_copies_from_hand(|card| card.golden_mult() as u32)
}
