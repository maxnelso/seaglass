//! `Wildfire Elemental` (`BGS_126`) — Tier 3 Elemental (`6/3`).
//! After this attacks and kills a minion, deal excess damage to an (`both` if Golden) adjacent enemy(ies).

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe};

pub const ID: CardId = 342;
pub const NAME: &str = "Wildfire Elemental";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 6, 3, 3).with_tribe(Tribe::Elemental)
}
