//! `Crackling Cyclone` (`BGS_119`) — Tier 1 Elemental (`2/1`).
//! **Divine Shield**, **Windfury**.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 108;
pub const NAME: &str = "Crackling Cyclone";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 1, 1)
        .with_tribe(Tribe::Elemental)
        .with_keyword(Keyword::DivineShield)
        .with_keyword(Keyword::Windfury)
}
