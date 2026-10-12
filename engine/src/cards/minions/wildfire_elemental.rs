//! `Wildfire Elemental` (`BGS_126`) — Tier 3 Elemental (`6/3`).
//! After this attacks and kills a minion, deal excess damage to an (`both` if Golden) adjacent enemy(ies).
//!
//! The excess is this minion's Attack minus the target's Health before the hit: none when the
//! target survives (or its Divine Shield absorbs the hit), and none when Venomous destroys a
//! target with more Health than that Attack.

use crate::cards::{CardFlags, CardTemplate};
use crate::model::{CardId, Tribe};

pub const ID: CardId = 342;
pub const NAME: &str = "Wildfire Elemental";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 6, 3, 3)
        .with_tribe(Tribe::Elemental)
        .with_flags(CardFlags::EXCESS_DAMAGE_TO_NEIGHBORS)
}
