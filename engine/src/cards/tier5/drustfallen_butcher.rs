//! `Drustfallen Butcher` (`BG32_324`) — Tier 5 Undead (`2/7`).
//!
//! Avenge (4): Get a (`2` if Golden) `Butchering`(s).

use crate::cards::{spells, sync_unit_auras, CardTemplate};
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 513;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Drustfallen Butcher", 2, 7, 5).with_tribe(Tribe::Undead)
}

pub fn on_friendly_death(
    unit: &mut Unit,
    hand: &mut Vec<Unit>,
    hand_summoned: &mut Vec<bool>,
    auras: &PlayerAuras,
) {
    if unit.card_id != ID || unit.health <= 0 {
        return;
    }
    unit.avenge_counter += 1;
    if unit.avenge_counter >= 4 {
        unit.avenge_counter -= 4;
        let count = if unit.is_golden { 2 } else { 1 };
        for _ in 0..count {
            if hand.len() < 10 {
                if let Some(mut spell) = spells::spell_by_id(spells::SPELL_BUTCHERING) {
                    sync_unit_auras(&mut spell, auras);
                    hand.push(spell);
                    hand_summoned.push(false);
                }
            }
        }
    }
}
