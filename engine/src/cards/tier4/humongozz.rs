//! `Humon'gozz` (`BG32_341`) — Tier 4 Neutral (`5/5`, Divine Shield).
//!
//! Divine Shield. Your Tavern spells give an extra `+1/+2` (`+2/+4` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Unit};

pub const ID: CardId = 432;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Humon'gozz", 5, 5, 4).with_keyword(Keyword::DivineShield)
}

pub fn init_spell_aura(unit: &mut Unit) {
    if unit.card_id == ID {
        let m = if unit.is_golden { 2 } else { 1 };
        unit.spell_atk_aura = m;
        unit.spell_hp_aura = 2 * m;
    }
}
