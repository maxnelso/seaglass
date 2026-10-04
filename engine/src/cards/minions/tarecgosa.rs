//! `Tarecgosa` (`BG21_015`) — Tier 2 Dragon (`4/4`).
//! Permanently keeps Bonus Keywords and (`double` if Golden) stats gained in combat.

use crate::cards::CardTemplate;
use crate::model::{CardId, Tribe, Unit};

pub const ID: CardId = 231;
pub const NAME: &str = "Tarecgosa";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 4, 4, 2)
        .with_tribe(Tribe::Dragon)
        .on_post_combat_keep_mult(Unit::golden_mult)
}
