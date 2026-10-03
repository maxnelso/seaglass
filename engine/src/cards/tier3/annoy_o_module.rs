//! `Annoy-o-Module` (`BG_BOT_911`) — Tier 3 Mech (`2/4`).
//! **Magnetic**, **Divine Shield**, **Taunt**.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 304;
pub const NAME: &str = "Annoy-o-Module";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 4, 3)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::Magnetic)
        .with_keyword(Keyword::DivineShield)
        .with_keyword(Keyword::Taunt)
}
