//! `Shield of the Legion` (`BG25_807t4`) — Tier 1 Mech/Demon (`7/3`), in no pool.
//! **Magnetic**. **Divine Shield**. Can **Magnetize** to Mechs *and* Demons.
//!
//! One of the four Mecha-Demons only `Mecha-Jaraxxus Sticker` gives (`docs/trinkets.md` §11.9):
//! a Mech and a Demon (`Tribe::MechDemon`), so it can Magnetize to either.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 10812;
pub const NAME: &str = "Shield of the Legion";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 7, 3, 1)
        .with_tribe(Tribe::MechDemon)
        .with_keyword(Keyword::Magnetic)
        .with_keyword(Keyword::DivineShield)
}
