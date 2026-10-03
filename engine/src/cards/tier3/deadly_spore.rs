//! `Deadly Spore` (`BGS_131`) — Tier 3 Neutral (`1/1`).
//! **Venomous**.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 309;
pub const NAME: &str = "Deadly Spore";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 1, 3)
        .with_tribe(Tribe::None)
        .with_keyword(Keyword::Venomous)
}
