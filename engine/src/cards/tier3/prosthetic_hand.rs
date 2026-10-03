//! `Prosthetic Hand` (`BG_DEEP_015`) — Tier 3 Undead/Mech (`3/1`).
//! **Magnetic**, **Reborn**. Can **Magnetize** to Mechs or Undead.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 326;
pub const NAME: &str = "Prosthetic Hand";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 1, 3)
        .with_tribe(Tribe::UndeadMech)
        .with_keyword(Keyword::Magnetic)
        .with_keyword(Keyword::Reborn)
}
