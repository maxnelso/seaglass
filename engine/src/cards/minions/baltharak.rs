//! `Baltharak` (`BG25_807t3`) — Tier 1 Mech/Demon (`10/1`), in no pool.
//! **Magnetic**. **Reborn**. Can **Magnetize** to Mechs *and* Demons.
//!
//! One of the four Mecha-Demons only `Mecha-Jaraxxus Sticker` gives (`docs/trinkets.md` §11.9):
//! a Mech and a Demon (`Tribe::MechDemon`), so it can Magnetize to either.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 10811;
pub const NAME: &str = "Baltharak";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 10, 1, 1)
        .with_tribe(Tribe::MechDemon)
        .with_keyword(Keyword::Magnetic)
        .with_keyword(Keyword::Reborn)
}
