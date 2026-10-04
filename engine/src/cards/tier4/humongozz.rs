//! `Humon'gozz` (`BG32_341`) — Tier 4 Neutral (`5/5`, Divine Shield).
//!
//! Divine Shield. Your Tavern spells give an extra `+1/+2` (`+2/+4` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword};

pub const ID: CardId = 432;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Humon'gozz", 5, 5, 4)
        .with_keyword(Keyword::DivineShield)
        .with_spell_aura(1, 2)
}
