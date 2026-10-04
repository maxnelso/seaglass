//! `Eternal Knight` (`BG25_008`) — Tier 2 Undead (`4/2`).
//! Has `+4/+2` (`+8/+4` if Golden) for each friendly `Eternal Knight` that died this game *(wherever this is)*.

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 209;
pub const NAME: &str = "Eternal Knight";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 2, 2)
        .with_tribe(Tribe::Undead)
        .on_died(on_died)
        .on_aura_bonus(aura_bonus)
}

/// `+4/+2` (`+8/+4` if Golden) for each friendly `Eternal Knight` that died this game (this
/// card's counter).
pub fn aura_bonus(unit: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    let stacks = auras.counter(ID) * unit.golden_mult();
    (4 * stacks, 2 * stacks)
}

/// Count this death towards every `Eternal Knight`'s aura.
pub fn on_died(_unit: &Unit, auras: &mut PlayerAuras) {
    auras.add_counter(ID, 1);
}
