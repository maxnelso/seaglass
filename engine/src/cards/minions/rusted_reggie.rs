//! `Rusted Reggie` (`BG25_807t`) — Tier 1 Mech/Demon (`5/5`), in no pool.
//! **Magnetic**. **Windfury**. Can **Magnetize** to Mechs *and* Demons.
//!
//! One of the four Mecha-Demons only `Mecha-Jaraxxus Sticker` gives (`docs/trinkets.md` §11.9):
//! a Mech and a Demon (`Tribe::MechDemon`), so it can Magnetize to either.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Tribe};

pub const ID: CardId = 10809;
pub const NAME: &str = "Rusted Reggie";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 5, 5, 1)
        .with_tribe(Tribe::MechDemon)
        .with_keyword(Keyword::Magnetic)
        .with_keyword(Keyword::Windfury)
}
