//! `Enchanted Sentinel` (`BG35_341`) — Tier 4 Mech (`3/5`, Magnetic).
//!
//! Magnetic. Your Tavern spells give an extra `+1/+1` (`+2/+2` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe, Unit};

pub const ID: CardId = 418;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Enchanted Sentinel", 3, 5, 4)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::Magnetic)
}

pub fn init_spell_aura(unit: &mut Unit) {
    if unit.card_id == ID {
        let m = if unit.is_golden { 2 } else { 1 };
        unit.spell_atk_aura = m;
        unit.spell_hp_aura = m;
    }
}
