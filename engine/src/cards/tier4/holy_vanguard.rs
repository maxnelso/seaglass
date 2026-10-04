//! `Holy Vanguard` (`BG36_372`) — Tier 4 Neutral (`10/10`, Divine Shield).
//!
//! Divine Shield. Has `+30/+30` (`+60/+60` if Golden) if you have 15 or less Health.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, PlayerAuras, Unit};

pub const ID: CardId = 430;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Holy Vanguard", 10, 10, 4)
        .with_keyword(Keyword::DivineShield)
        .on_aura_bonus(aura_bonus)
}

/// `+30/+30` (`+60/+60` if Golden) while the hero has 15 or less Health.
pub fn aura_bonus(unit: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    if auras.hero_low_health {
        (30 * unit.golden_mult(), 30 * unit.golden_mult())
    } else {
        (0, 0)
    }
}
