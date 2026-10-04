//! `Enchanted Sentinel` (`BG35_341`) — Tier 4 Mech (`3/5`, Magnetic).
//!
//! Magnetic. Your Tavern spells give an extra `+1/+1` (`+2/+2` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 418;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Enchanted Sentinel", 3, 5, 4)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::Magnetic)
        .with_spell_aura(1, 1)
}
