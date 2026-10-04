//! `Falling Sky Golem` (`BG35_342`) — Tier 6 Mech (`4/2`, Divine Shield).
//!
//! Divine Shield. Has `+4/+2` (`+8/+4` if Golden) for each Deathrattle you've triggered this game (wherever this is).

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 610;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Falling Sky Golem", 4, 2, 6)
        .with_tribe(Tribe::Mech)
        .with_keyword(Keyword::DivineShield)
}

pub fn sync_aura(unit: &mut Unit, auras: &PlayerAuras) {
    if unit.card_id != ID {
        return;
    }
    if auras.deathrattles_triggered > unit.sky_golem_stacks_applied {
        let diff = (auras.deathrattles_triggered - unit.sky_golem_stacks_applied) as i32;
        unit.sky_golem_stacks_applied = auras.deathrattles_triggered;
        let mult = if unit.is_golden { 2 } else { 1 };
        unit.add_stats(4 * mult * diff, 2 * mult * diff);
    }
}
