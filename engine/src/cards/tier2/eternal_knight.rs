//! `Eternal Knight` (`BG25_008`) — Tier 2 Undead (`4/2`).
//! Has `+4/+2` (`+8/+4` if Golden) for each friendly `Eternal Knight` that died this game *(wherever this is)*.

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 209;
pub const NAME: &str = "Eternal Knight";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 2, 2).with_tribe(Tribe::Undead)
}

/// Synchronize this unit's `Eternal Knight` death-count aura with `auras.eternal_knights_died`.
pub fn sync_unit(unit: &mut Unit, auras: &PlayerAuras) {
    if unit.card_id != ID {
        return;
    }
    let target_stacks = auras.eternal_knights_died;
    if target_stacks > unit.eternal_knight_stacks_applied {
        let diff = (target_stacks - unit.eternal_knight_stacks_applied) as i32;
        let mult = if unit.is_golden { 2 } else { 1 };
        unit.eternal_knight_stacks_applied = target_stacks;
        unit.add_stats(4 * mult * diff, 2 * mult * diff);
    }
}
