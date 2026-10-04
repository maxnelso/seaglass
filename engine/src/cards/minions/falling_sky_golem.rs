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
        .on_aura_bonus(aura_bonus)
}

/// `+4/+2` (`+8/+4` if Golden) for each Deathrattle triggered this game.
pub fn aura_bonus(unit: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    let stacks = auras.deathrattles_triggered as i32 * unit.golden_mult();
    (4 * stacks, 2 * stacks)
}
