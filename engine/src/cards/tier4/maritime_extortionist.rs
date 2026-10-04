//! `Maritime Extortionist` (`BG36_524`) — Tier 4 Pirate (`7/7`).
//!
//! Has `+7/+7` (`+14/+14` if Golden) for each Golden minion you've played this game (wherever this is).

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 440;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Maritime Extortionist", 7, 7, 4)
        .with_tribe(Tribe::Pirate)
        .on_aura_bonus(aura_bonus)
}

/// `+7/+7` (`+14/+14` if Golden) for each Golden minion played this game.
pub fn aura_bonus(unit: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    let bonus = auras.golden_minions_played as i32 * 7 * unit.golden_mult();
    (bonus, bonus)
}
