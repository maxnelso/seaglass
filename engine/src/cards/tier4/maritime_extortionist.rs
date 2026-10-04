//! `Maritime Extortionist` (`BG36_524`) — Tier 4 Pirate (`7/7`).
//!
//! Has `+7/+7` (`+14/+14` if Golden) for each Golden minion you've played this game (wherever this is).

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 440;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Maritime Extortionist", 7, 7, 4)
        .with_tribe(Tribe::Pirate)
        .on_sync_aura(sync_unit)
}

pub fn sync_unit(unit: &mut Unit, auras: &PlayerAuras) {
    let diff = auras.golden_minions_played as i32 - unit.maritime_stacks_applied as i32;
    if diff != 0 {
        let per_stack = if unit.is_golden { 14 } else { 7 };
        unit.maritime_stacks_applied = auras.golden_minions_played;
        unit.add_stats(diff * per_stack, diff * per_stack);
    }
}
