//! `Risen Rider` (`BG25_001`) — Tier 1 Undead (`2/1`).
//! **Taunt**, **Reborn**.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 119;
pub const NAME: &str = "Risen Rider";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 1, 1)
        .with_tribe(Tribe::Undead)
        .with_keyword(Keyword::Taunt)
        .with_keyword(Keyword::Reborn)
}
