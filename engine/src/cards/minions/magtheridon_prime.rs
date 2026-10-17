//! `Magtheridon Prime` (`BG25_807t2`) — Tier 1 Mech/Demon (`1/10`), in no pool.
//! **Magnetic**. **Taunt**. Can **Magnetize** to Mechs *and* Demons.
//!
//! One of the four Mecha-Demons only `Mecha-Jaraxxus Sticker` gives (`docs/trinkets.md` §11.9):
//! a Mech and a Demon (`Tribe::MechDemon`), so it can Magnetize to either.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 10810;
pub const NAME: &str = "Magtheridon Prime";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 10, 1)
        .with_tribe(Tribe::MechDemon)
        .with_keyword(Keyword::Magnetic)
        .with_keyword(Keyword::Taunt)
}
