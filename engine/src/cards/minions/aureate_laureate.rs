//! `Aureate Laureate` (`BG32_236`) — Tier 1 Pirate (`2/2`).
//! **Divine Shield**. This minion is always Golden, but doesn't give a Triple Reward.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 114;
pub const NAME: &str = "Aureate Laureate";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 2, 1)
        .with_tribe(Tribe::Pirate)
        .with_keyword(Keyword::DivineShield)
        .with_intrinsic_golden()
}
