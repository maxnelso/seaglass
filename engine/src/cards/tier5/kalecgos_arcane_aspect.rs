//! `Kalecgos, Arcane Aspect` (`BGS_041`) — Tier 5 Dragon (`4/12`).
//!
//! After you trigger a Battlecry, give your Dragons `+2/+2` (`+4/+4` if Golden).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};
use crate::tavern::TavernState;

pub const ID: CardId = 528;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Kalecgos, Arcane Aspect", 4, 12, 5).with_tribe(Tribe::Dragon)
}

pub fn after_battlecry_triggered(state: &mut TavernState, played_unit: &mut Unit) {
    let mut buff = 0i32;
    for u in &state.board {
        if u.card_id == ID {
            buff += if u.is_golden { 4 } else { 2 };
        }
    }
    if buff > 0 {
        for u in &mut state.board {
            if u.tribe.matches(Tribe::Dragon) {
                u.add_stats(buff, buff);
            }
        }
        if played_unit.tribe.matches(Tribe::Dragon) {
            played_unit.add_stats(buff, buff);
        }
    }
}
