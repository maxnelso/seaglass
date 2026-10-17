//! `Flourishing Frostling` (`BG26_537`) — Tier 5 Elemental (`2/1`), in no pool.
//! Has `+2/+1` (`+4/+2` if Golden) for each Elemental you played this game *(wherever this is)*.
//!
//! Only `Jarred Frostling` summons it (`docs/trinkets.md` §11.9). Its aura counts the
//! Elementals the player played this game (`PlayerAuras::elementals_played`), from the start of
//! the game.

use crate::cards::CardTemplate;
use crate::model::{CardId, PlayerAuras, Tribe, Unit};

pub const ID: CardId = 10805;
pub const NAME: &str = "Flourishing Frostling";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 2, 1, 5)
        .with_tribe(Tribe::Elemental)
        .on_aura_bonus(aura_bonus)
}

/// `+2/+1` (`+4/+2` if Golden) per Elemental played this game.
pub fn aura_bonus(unit: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    let stacks = (auras.elementals_played as i32).saturating_mul(unit.golden_mult());
    (stacks.saturating_mul(2), stacks)
}
