//! `Elemental of Surprise` (`BG26_175`) — Tier 6 Elemental (`8/8`, Divine Shield).
//!
//! Divine Shield. This minion can triple with any Elemental.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 608;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Elemental of Surprise", 8, 8, 6)
        .with_tribe(Tribe::Elemental)
        .with_keyword(Keyword::DivineShield)
}
