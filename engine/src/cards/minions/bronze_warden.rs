//! `Bronze Warden` (`BGS_034`) — Tier 2 Dragon (`2/1`).
//! **Divine Shield**, **Reborn**.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 204;
pub const NAME: &str = "Bronze Warden";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 1, 2)
        .with_tribe(Tribe::Dragon)
        .with_keyword(Keyword::DivineShield)
        .with_keyword(Keyword::Reborn)
}
